use crate::{
    steam::{self, Steam},
    vendor::{self, Progress},
};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::Serialize;
use serde_json::json;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug)]
pub struct Options {
    pub steam_root: Option<PathBuf>,
    pub data: PathBuf,
}
impl Options {
    pub fn new(steam_root: Option<PathBuf>, data: Option<PathBuf>) -> Result<Self> {
        let data = data.unwrap_or(
            dirs::data_local_dir()
                .context("No user data directory")?
                .join("linux-game-bridge"),
        );
        ensure!(data.is_absolute(), "Data directory must be absolute");
        Ok(Self { steam_root, data })
    }
    pub fn steam(&self) -> Result<Steam> {
        steam::detect(self.steam_root.as_deref())
    }
    fn lock(&self) -> Result<File> {
        fs::create_dir_all(&self.data)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.data.join("operation.lock"))?;
        file.try_lock_exclusive()
            .context("Linux Game Bridge is already setting up, updating, or running a game")?;
        Ok(file)
    }
}

#[derive(Debug, Serialize)]
pub struct Doctor {
    pub steam: Steam,
    pub launcher_installed: bool,
    pub prefix_initialized: bool,
    pub mod_installed: bool,
    pub messages: Vec<String>,
}
pub fn doctor(options: &Options) -> Result<Doctor> {
    let steam = options.steam()?;
    let mut messages = Vec::new();
    if !steam.configured {
        messages.push("Prepare Steam to enable Proton Experimental for ETS2.".into());
    }
    if steam.windows_game().is_none() {
        messages.push("Steam must finish downloading the Windows version of ETS2.".into());
    }
    if steam.proton.is_none() {
        messages.push("Proton Experimental is not installed yet.".into());
    }
    if steam.runtime.is_none() {
        messages.push("Steam Linux Runtime 4 is not installed yet.".into());
    }
    if steam.game_running {
        messages.push("ETS2 is running. Close the game before setup or updates.".into());
    }
    Ok(Doctor {
        launcher_installed: vendor::launcher(&options.data).is_ok(),
        prefix_initialized: steam.prefix().join("pfx/user.reg").is_file(),
        mod_installed: options.data.join("TruckersMP/core_ets2mp.dll").is_file(),
        steam,
        messages,
    })
}

pub fn prepare(options: &Options, progress: Progress<'_>) -> Result<()> {
    let _lock = options.lock()?;
    steam::configure(&options.steam()?, &options.data, progress)
}

pub fn fetch(options: &Options, installer: Option<&Path>, progress: Progress<'_>) -> Result<()> {
    let _lock = options.lock()?;
    ensure!(
        !steam::process_running("eurotrucks2.exe"),
        "Close ETS2 before downloading or updating mod files"
    );
    ensure!(
        !steam::launcher_running(),
        "Close the official TruckersMP launcher before setup or updates"
    );
    let client = vendor::client()?;
    vendor::install_launcher(&client, &options.data, installer, progress)?;
    vendor::update_mod(&client, &options.data.join("TruckersMP"), progress)?;
    Ok(())
}

pub fn install(options: &Options, progress: Progress<'_>) -> Result<()> {
    let _lock = options.lock()?;
    let steam = options.steam()?;
    ensure!(!steam.game_running, "Close ETS2 before installation");
    ensure!(
        steam.ready(),
        "Steam setup is incomplete. Choose Prepare Steam, finish its downloads, close ETS2, then choose Install."
    );
    ensure!(steam.running, "Open Steam and sign in before installing");
    ensure!(
        !steam::launcher_running(),
        "Close the official TruckersMP launcher before setup or updates"
    );
    let client = vendor::client()?;
    let launcher = vendor::install_launcher(&client, &options.data, None, progress)?;
    vendor::update_mod(&client, &options.data.join("TruckersMP"), progress)?;
    if !steam.prefix().join("pfx/user.reg").is_file() {
        progress("Creating ETS2's Windows compatibility environment…".into());
        fs::create_dir_all(steam.prefix())?;
        let output = steam::proton_command(&steam)?
            .args(["wineboot", "-u"])
            .output()?;
        ensure!(
            output.status.success(),
            "Proton could not create the game environment. Start ETS2 through Steam once, close it, and retry Install."
        );
    }
    configure_launcher(options, &steam, progress)?;
    ensure!(
        launcher.join("TruckersMP-Launcher.exe").is_file(),
        "Launcher installation did not finish"
    );
    progress(
        "Setup complete. Play will check for current multiplayer files before every launch.".into(),
    );
    Ok(())
}

fn configure_launcher(options: &Options, steam: &Steam, progress: Progress<'_>) -> Result<()> {
    let settings_dir = steam
        .prefix()
        .join("pfx/drive_c/users/steamuser/AppData/Roaming/TruckersMP");
    let path = settings_dir.join("launcher-options.json");
    let mut settings = if path.exists() {
        let contents = fs::read(&path)?;
        let backup = options
            .data
            .join("backups/launcher-options-before-truckbridge.json");
        if !backup.exists() {
            vendor::atomic_write(&backup, &contents)?;
        }
        serde_json::from_slice(&contents)
            .context("Existing TruckersMP settings are invalid; they were backed up")?
    } else {
        json!({})
    };
    ensure!(
        settings.is_object(),
        "Existing launcher settings are not an object"
    );
    ensure!(
        settings
            .get("games")
            .is_none_or(|g| g.is_null() || g.is_object()),
        "Invalid games settings; existing file was backed up"
    );
    ensure!(
        settings["games"]
            .get("ets2")
            .is_none_or(|g| g.is_null() || g.is_object()),
        "Invalid ETS2 settings; existing file was backed up"
    );
    settings["games"]["ets2"]["game"] = json!("ets2");
    settings["games"]["ets2"]["path"] = json!(steam::windows_path(
        &steam.windows_game().context("Windows ETS2 is missing")?
    )?);
    if !settings["games"]["ets2"]["consoleOpts"].is_array() {
        settings["games"]["ets2"]["consoleOpts"] = json!([]);
    }
    if !settings["games"]["ats"].is_object() {
        settings["games"]["ats"] =
            json!({"game":"ats", "path":"C:\\NotInstalled\\amtrucks.exe", "consoleOpts":[]});
    }
    ensure!(
        settings["games"]["ets2"]["consoleOpts"]
            .as_array()
            .is_some_and(|v| v.iter().all(|x| x.is_string())),
        "ETS2 launch options must be strings; existing settings were backed up"
    );
    let ats = &settings["games"]["ats"];
    ensure!(
        ats["game"].as_str() == Some("ats")
            && ats["path"].is_string()
            && ats["consoleOpts"]
                .as_array()
                .is_some_and(|v| v.iter().all(|x| x.is_string())),
        "Existing ATS settings are invalid; they were backed up before any change"
    );
    if !settings["theme"]
        .as_str()
        .is_none_or(|v| ["system", "dark", "light"].contains(&v))
        || (!settings["theme"].is_null() && !settings["theme"].is_string())
    {
        settings.as_object_mut().unwrap().remove("theme");
    }
    settings["installPath"] = json!(steam::windows_path(&options.data.join("TruckersMP"))?);
    settings["closeOnLaunch"] = json!(true);
    vendor::atomic_write(&path, &serde_json::to_vec_pretty(&settings)?)?;
    progress("Configured the official launcher for your Steam library and game environment".into());
    Ok(())
}

pub fn update(options: &Options, progress: Progress<'_>) -> Result<()> {
    let _lock = options.lock()?;
    ensure!(
        !steam::process_running("eurotrucks2.exe"),
        "Close ETS2 before updating"
    );
    ensure!(
        !steam::launcher_running(),
        "Close the official TruckersMP launcher before updating"
    );
    vendor::update_mod(
        &vendor::client()?,
        &options.data.join("TruckersMP"),
        progress,
    )?;
    Ok(())
}

pub fn play(options: &Options, launcher_only: bool, progress: Progress<'_>) -> Result<()> {
    let _lock = options.lock()?;
    let steam = options.steam()?;
    ensure!(
        !steam.game_running,
        "ETS2 is already running. Use the existing game window."
    );
    ensure!(
        steam.ready(),
        "Steam prerequisites are missing. Choose Prepare Steam and complete its downloads."
    );
    ensure!(
        steam.running,
        "Steam is closed. Open Steam, sign in, then choose Play again."
    );
    ensure!(
        steam.prefix().join("pfx/user.reg").is_file(),
        "Choose Install first to finish setting up TruckersMP"
    );
    let launcher = vendor::launcher(&options.data).context("Install TruckersMP first")?;
    ensure!(
        !steam::launcher_running(),
        "TruckersMP launcher is already open; use its existing window"
    );
    let version = vendor::update_mod(
        &vendor::client()?,
        &options.data.join("TruckersMP"),
        progress,
    )?;
    let game_version = steam::game_version(&steam.windows_game().context("Windows ETS2 missing")?)?;
    ensure!(
        game_version == version.supported_game_version.trim_end_matches('s'),
        "ETS2 {game_version} is installed, but TruckersMP requires {}. Select the supported Steam beta branch from TruckersMP's version guide, then try again.",
        version.supported_game_version
    );
    progress(format!(
        "Current mod: {}; requires ETS2 {}. Starting official launcher…",
        version.name, version.supported_game_version
    ));
    configure_launcher(options, &steam, progress)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(options.data.join("launcher.log"))?;
    let mut command = steam::proton_command(&steam)?;
    command
        .arg(launcher.join("TruckersMP-Launcher.exe"))
        .current_dir(&launcher)
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    if !launcher_only {
        command.arg("--quicklaunch=ets2");
    }
    let status = command.spawn()?.wait()?;
    ensure!(
        status.success(),
        "Official launcher exited with {status}. Details: {}",
        options.data.join("launcher.log").display()
    );
    progress("Launcher session ended. If a server refuses the game version, use the official launcher or Steam's supported beta branch.".into());
    Ok(())
}

pub fn install_desktop(options: &Options) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let _lock = options.lock()?;
    let bin_dir = dirs::data_local_dir()
        .context("No user data directory")?
        .join("linux-game-bridge/bin");
    fs::create_dir_all(&bin_dir)?;
    let dest = bin_dir.join("linux-game-bridge");
    let source = std::env::current_exe()?;
    if source.canonicalize()? != dest.canonicalize().unwrap_or_default() {
        vendor::atomic_write(&dest, &fs::read(source)?)?;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
    }
    let directory = dirs::data_local_dir().unwrap().join("applications");
    let extra = if let Some(root) = &options.steam_root {
        format!(" --steam-root {}", desktop_arg(root)?)
    } else {
        String::new()
    };
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Linux Game Bridge\nComment=Set up and play official TruckersMP on Linux\nExec={} --data-dir {}{}\nIcon=applications-games\nTerminal=false\nCategories=Game;Simulation;\nStartupNotify=true\n",
        desktop_arg(&dest)?,
        desktop_arg(&options.data)?,
        extra
    );
    let path = directory.join("linux-game-bridge.desktop");
    vendor::atomic_write(&path, entry.as_bytes())?;
    let _ = Command::new("update-desktop-database")
        .arg(&directory)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(path)
}
fn desktop_arg(path: &Path) -> Result<String> {
    let text = path.to_str().context("Desktop path is not UTF-8")?;
    ensure!(!text.contains(['\n', '\r']), "Invalid desktop path");
    // Desktop Exec is not a shell: percent is a field-code marker, and quoted reserved characters must be escaped.
    Ok(format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path) -> (Options, Steam, PathBuf) {
        let game = root.join("steamapps/common/Euro Truck Simulator 2");
        fs::create_dir_all(game.join("bin/win_x64")).unwrap();
        fs::write(game.join("bin/win_x64/eurotrucks2.exe"), b"MZ").unwrap();
        let steam = Steam {
            root: root.into(),
            libraries: vec![root.into()],
            game: Some(game),
            proton: None,
            runtime: None,
            running: false,
            game_running: false,
            configured: true,
        };
        let options = Options::new(None, Some(root.join("app"))).unwrap();
        let settings = steam
            .prefix()
            .join("pfx/drive_c/users/steamuser/AppData/Roaming/TruckersMP/launcher-options.json");
        (options, steam, settings)
    }
    #[test]
    fn configures_paths_and_preserves_user_launch_options() {
        let temp = tempfile::tempdir().unwrap();
        let (options, steam, path) = fixture(temp.path());
        vendor::atomic_write(
            &path,
            br#"{"games":{"ets2":{"consoleOpts":["-nointro"]}},"theme":"dark"}"#,
        )
        .unwrap();
        configure_launcher(&options, &steam, &|_| {}).unwrap();
        let settings: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            settings["games"]["ets2"]["consoleOpts"],
            json!(["-nointro"])
        );
        assert_eq!(settings["theme"], "dark");
        assert!(
            settings["games"]["ets2"]["path"]
                .as_str()
                .unwrap()
                .ends_with("bin\\win_x64\\eurotrucks2.exe")
        );
        assert!(
            options
                .data
                .join("backups/launcher-options-before-truckbridge.json")
                .is_file()
        );
    }
    #[test]
    fn malformed_existing_settings_are_backed_up_without_overwrite_or_panic() {
        let temp = tempfile::tempdir().unwrap();
        let (options, steam, path) = fixture(temp.path());
        let original = br#"{"games":"invalid"}"#;
        vendor::atomic_write(&path, original).unwrap();
        assert!(configure_launcher(&options, &steam, &|_| {}).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}
