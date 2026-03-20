use clap::Parser;

use cctrack::{collector, config::Config, store, tui, web};
use store::event::StoreSnapshot;

#[derive(Parser)]
#[command(name = "cctrack", version, about = "Real-time observability for Claude Code agent teams")]
struct Cli {
    /// Monitor a specific team
    #[arg(long)]
    team: Option<String>,

    /// Also start web UI
    #[arg(long)]
    web: bool,

    /// Web UI only, no TUI
    #[arg(long)]
    web_only: bool,

    /// Web UI port
    #[arg(long, default_value = "7891")]
    port: u16,

    /// Use light theme (for light terminal backgrounds)
    #[arg(long)]
    light: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Manage Claude Code hooks
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
}

#[derive(clap::Subcommand)]
enum HooksAction {
    /// Install hooks into Claude Code settings
    Install,
    /// Remove hooks from Claude Code settings
    Uninstall,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = Config::load();

    // Initialize tracing for web-only mode (TUI mode can't use stdout logging)
    if cli.web_only {
        tracing_subscriber::fmt::init();
    }

    // Handle subcommands
    match &cli.command {
        Some(Commands::Hooks { action }) => {
            let claude_home = Config::claude_home();
            match action {
                HooksAction::Install => {
                    match collector::hooks_installer::install_hooks(&claude_home, config.hooks.port)
                    {
                        Ok(()) => println!("Hooks installed successfully."),
                        Err(e) => eprintln!("Failed to install hooks: {e}"),
                    }
                }
                HooksAction::Uninstall => {
                    match collector::hooks_installer::uninstall_hooks(&claude_home) {
                        Ok(()) => println!("Hooks removed."),
                        Err(e) => eprintln!("Failed to remove hooks: {e}"),
                    }
                }
            }
            return Ok(());
        }
        None => {}
    }

    // Set theme mode
    tui::theme::set_light_mode(cli.light);

    // Auto-install hooks on first run
    let claude_home = Config::claude_home();
    if config.hooks.auto_install {
        let _ = collector::hooks_installer::install_hooks(&claude_home, config.hooks.port);
    }

    // Main monitoring mode
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(256);
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(StoreSnapshot::default());

    // Start store processor
    tokio::spawn(store::state::Store::process_events(event_rx, snapshot_tx));

    // Start collectors
    tokio::spawn(collector::file_watcher::run(
        claude_home.clone(),
        event_tx.clone(),
    ));
    tokio::spawn(collector::hook_server::run(
        config.hooks.port,
        event_tx.clone(),
    ));

    // Determine web port
    let web_port = if cli.port != 7891 {
        cli.port
    } else {
        config.web.port
    };

    // Always start web server in background
    let web_rx = snapshot_rx.clone();
    tokio::spawn(web::run(web_port, web_rx));

    // Start TUI (or wait if web-only)
    if cli.web_only {
        println!("cctrack web UI: http://localhost:{web_port}");
        println!("Press Ctrl+C to stop.");
        tokio::signal::ctrl_c().await?;
    } else {
        tui::run_tui(snapshot_rx).await?;
    }

    Ok(())
}
