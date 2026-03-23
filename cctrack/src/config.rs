use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            web: WebConfig::default(),
            hooks: HooksConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default)]
    pub enabled: bool,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            port: default_web_port(),
            enabled: false,
        }
    }
}

fn default_web_port() -> u16 {
    7891
}

#[derive(Debug, Deserialize)]
pub struct HooksConfig {
    #[serde(default = "default_true")]
    pub auto_install: bool,
    #[serde(default = "default_hooks_port")]
    pub port: u16,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            auto_install: true,
            port: default_hooks_port(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_hooks_port() -> u16 {
    7890
}

#[derive(Debug, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
        }
    }
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match path {
            Some(p) if p.exists() => {
                let content = std::fs::read_to_string(&p).unwrap_or_default();
                toml::from_str(&content).unwrap_or_default()
            }
            _ => Config::default(),
        }
    }

    pub fn claude_home() -> PathBuf {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".claude")
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cctrack/config.toml"))
}
