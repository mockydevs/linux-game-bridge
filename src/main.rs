mod gui;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use linux_game_bridge::engine::{self, Options};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Unofficial Linux setup assistant for the official TruckersMP client"
)]
struct Cli {
    /// Native Steam installation folder (auto-detected by default)
    #[arg(long, global = true)]
    steam_root: Option<PathBuf>,
    /// App data folder (default: XDG_DATA_HOME/truckbridge)
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[arg(long, hide = true)]
    smoke_ui: bool,
    #[command(subcommand)]
    command: Option<Action>,
}
#[derive(Subcommand)]
enum Action {
    /// Inspect prerequisites without changing settings or contacting servers
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Back up settings, enable Proton for ETS2, and let Steam install dependencies
    Prepare,
    /// Download and configure the official launcher and multiplayer mod
    Install,
    /// Download/verify vendor files only; does not configure Steam or its game prefix
    Fetch {
        /// Use an already-downloaded official Squirrel installer
        #[arg(long)]
        installer: Option<PathBuf>,
    },
    /// Verify all official mod files and download changed files
    Update,
    /// Update the mod, then start the game using the official launcher
    Play,
    /// Update the mod, then open the official launcher's settings and updater
    Launcher,
    /// Install this executable in your user directory and add a desktop-menu entry
    InstallDesktop,
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Linux Game Bridge: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    anyhow::ensure!(
        std::env::consts::OS == "linux" && std::env::consts::ARCH == "x86_64",
        "This build supports Linux x86_64 only"
    );
    let cli = Cli::parse();
    let options = Options::new(cli.steam_root, cli.data_dir)?;
    let progress = |message: String| println!("{message}");
    match cli.command {
        Some(Action::Doctor { json }) => {
            let report = engine::doctor(&options)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Steam: {}\nWindows game: {}\nProton: {}\nRuntime: {}\nLauncher installed: {}",
                    report.steam.root.display(),
                    report.steam.windows_game().is_some(),
                    report.steam.proton.is_some(),
                    report.steam.runtime.is_some(),
                    report.launcher_installed
                );
                for message in report.messages {
                    println!("• {message}");
                }
            }
            Ok(())
        }
        Some(Action::Prepare) => engine::prepare(&options, &progress),
        Some(Action::Install) => engine::install(&options, &progress),
        Some(Action::Fetch { installer }) => {
            engine::fetch(&options, installer.as_deref(), &progress)
        }
        Some(Action::Update) => engine::update(&options, &progress),
        Some(Action::Play) => engine::play(&options, false, &progress),
        Some(Action::Launcher) => engine::play(&options, true, &progress),
        Some(Action::InstallDesktop) => {
            println!("Installed {}", engine::install_desktop(&options)?.display());
            Ok(())
        }
        None => {
            let native = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_inner_size([820.0, 900.0])
                    .with_min_inner_size([620.0, 640.0]),
                ..Default::default()
            };
            eframe::run_native(
                "Linux Game Bridge",
                native,
                Box::new(move |cc| Ok(Box::new(gui::App::new(cc, options, cli.smoke_ui)))),
            )
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("Could not open desktop window. Use 'linux-game-bridge doctor' for CLI diagnostics.")
        }
    }
}
