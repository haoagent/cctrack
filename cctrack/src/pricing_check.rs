//! Compare cctrack's hardcoded pricing against LiteLLM's model pricing database.
//!
//! Fetches https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json
//! and compares with our get_pricing() for each Claude model family.

use std::collections::HashMap;

const LITELLM_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// Models we want to check (LiteLLM key prefix → our model key).
const MODELS: &[(&str, &str)] = &[
    ("claude-sonnet-4", "sonnet"),
    ("claude-opus-4", "opus"),
    ("claude-haiku-4", "haiku"),
];

#[derive(Debug, serde::Deserialize)]
struct LiteLLMModel {
    input_cost_per_token: Option<f64>,
    output_cost_per_token: Option<f64>,
    input_cost_per_token_above_200k_tokens: Option<f64>,
    output_cost_per_token_above_200k_tokens: Option<f64>,
    cache_creation_input_token_cost: Option<f64>,
    cache_read_input_token_cost: Option<f64>,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Fetching LiteLLM pricing database...");

    let resp = reqwest::get(LITELLM_URL).await?;
    if !resp.status().is_success() {
        eprintln!("Failed to fetch LiteLLM pricing: HTTP {}", resp.status());
        return Ok(());
    }

    let data: HashMap<String, LiteLLMModel> = resp.json().await?;
    println!("Loaded {} models from LiteLLM\n", data.len());

    let mut all_ok = true;

    for &(litellm_prefix, our_key) in MODELS {
        // Find the best matching LiteLLM entry (direct API, not bedrock/vertex)
        let entry = data.iter()
            .find(|(k, _)| k.starts_with(litellm_prefix) && !k.contains("anthropic.") && !k.contains("vertex") && !k.contains("bedrock"))
            .or_else(|| data.iter().find(|(k, _)| k.starts_with(litellm_prefix)));

        let Some((litellm_key, litellm)) = entry else {
            println!("⚠  {our_key}: no LiteLLM entry found for '{litellm_prefix}*'");
            continue;
        };

        let ours = cctrack::stats::get_pricing(our_key);

        println!("━━━ {} (LiteLLM: {}) ━━━", our_key.to_uppercase(), litellm_key);

        let checks = [
            ("input",        ours.input,           per_mtok(litellm.input_cost_per_token)),
            ("input >200k",  ours.input_above_200k, per_mtok(litellm.input_cost_per_token_above_200k_tokens)),
            ("output",       ours.output,           per_mtok(litellm.output_cost_per_token)),
            ("output >200k", ours.output_above_200k, per_mtok(litellm.output_cost_per_token_above_200k_tokens)),
            ("cache write",  ours.cache_write,      per_mtok(litellm.cache_creation_input_token_cost)),
            ("cache read",   ours.cache_read,       per_mtok(litellm.cache_read_input_token_cost)),
        ];

        for (field, ours_val, litellm_val) in checks {
            let litellm_val = match litellm_val {
                Some(v) => v,
                None => {
                    println!("  {field:14}  cctrack=${ours_val:<8.2}  litellm=N/A");
                    continue;
                }
            };

            let diff = (ours_val - litellm_val).abs();
            let ok = diff < 0.01;
            let icon = if ok { "✅" } else { "❌" };
            if !ok { all_ok = false; }

            println!("  {icon} {field:14}  cctrack=${ours_val:<8.2}  litellm=${litellm_val:<8.2}{}",
                if !ok { format!("  DIFF={diff:.2}") } else { String::new() });
        }
        println!();
    }

    if all_ok {
        println!("✅ All pricing is up-to-date!");
    } else {
        println!("❌ Some prices differ — update get_pricing() in stats.rs");
    }

    Ok(())
}

fn per_mtok(cost_per_token: Option<f64>) -> Option<f64> {
    cost_per_token.map(|c| c * 1_000_000.0)
}
