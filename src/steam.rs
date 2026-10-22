use crate::vendor::{self, Progress};
use anyhow::{Context, Result, bail, ensure};
use keyvalues_parser::{Obj, Value};
use serde::Serialize;
use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const ETS2: &str = "227300";
#[derive(Debug, Clone, Serialize)]
pub struct Steam {
    pub root: PathBuf,
    pub libraries: Vec<PathBuf>,
    pub game: Option<PathBuf>,
    pub proton: Option<PathBuf>,
    pub runtime: Option<PathBuf>,
    pub running: bool,
    pub game_running: bool,
    pub configured: bool,
}

fn child<'a>(value: &'a Value<'_>, key: &str) -> Option<&'a Value<'a>> {
    value.get_obj()?.get(key)?.first()
}
fn text(value: &Value<'_>, key: &str) -> Option<String> {
    child(value, key)?.get_str().map(ToString::to_string)
}

pub fn library_paths(source: &str, root: &Path) -> Result<Vec<PathBuf>> {
    let vdf = keyvalues_parser::parse(source).context("Cannot parse Steam libraryfolders.vdf")?;
    let obj = vdf.value.get_obj().context("Invalid Steam library list")?;
    let mut paths = vec![root.to_path_buf()];
    for values in obj.values() {
        for value in values {
            if let Some(path) = text(value, "path") {
                let path = PathBuf::from(path);
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
    }
    Ok(paths)
}

pub fn detect(override_root: Option<&Path>) -> Result<Steam> {
    let home = dirs::home_dir().context("No home directory")?;
    let candidates = if let Some(root) = override_root {
        vec![root.to_path_buf()]
    } else {
        vec![
            dirs::data_local_dir()
                .unwrap_or_else(|| home.join(".local/share"))
                .join("Steam"),
            home.join(".steam/steam"),
            home.join(".steam/root"),
        ]
    };
    let root = candidates
        .into_iter()
        .find(|p| p.join("steamapps").is_dir());
    let Some(root) = root else {
        if home.join(".var/app/com.valvesoftware.Steam").exists() {
            bail!(
                "Flatpak Steam detected. This first release supports native Steam only. Install your distribution's Steam package, then sign in."
            );
        }
        bail!(
            "Native Steam not found. Install Steam from your distribution's software manager, sign in, and install Euro Truck Simulator 2."
        );
    };
    let root = root.canonicalize()?;
    ensure!(
        !root.to_string_lossy().contains(".var/app/"),
        "Flatpak Steam is not supported in this release"
    );
    let libraries = library_paths(
        &fs::read_to_string(root.join("steamapps/libraryfolders.vdf"))?,
        &root,
    )?;
    let game = libraries.iter().find_map(|lib| {
        let manifest = fs::read_to_string(lib.join("steamapps/appmanifest_227300.acf")).ok()?;
        let parsed = keyvalues_parser::parse(&manifest).ok()?;
        let folder = text(&parsed.value, "installdir")?;
        let relative = vendor::safe_relative(&folder).ok()?;
        let game = lib.join("steamapps/common").join(relative);
        game.is_dir().then_some(game)
    });
    let proton = libraries
        .iter()
        .map(|p| p.join("steamapps/common/Proton - Experimental/proton"))
        .find(|p| p.is_file());
    let runtime = libraries
        .iter()
        .map(|p| p.join("steamapps/common/SteamLinuxRuntime_4/run"))
        .find(|p| p.is_file());
    let config = fs::read_to_string(root.join("config/config.vdf"))?;
    let parsed = keyvalues_parser::parse(&config)?;
    let mut value = Some(&parsed.value);
    for key in [
        "Software",
        "Valve",
        "Steam",
        "CompatToolMapping",
        ETS2,
        "name",
    ] {
        value = value.and_then(|v| child(v, key));
    }
    let configured = value
        .and_then(|v| v.get_str())
        .is_some_and(|s| s == "proton_experimental");
    Ok(Steam {
        root,
        libraries,
        game,
        proton,
        runtime,
        running: process_running("steam"),
        game_running: process_running("eurotrucks2.exe"),
        configured,
    })
}

impl Steam {
    pub fn windows_game(&self) -> Option<PathBuf> {
        self.game
            .as_ref()
            .map(|p| p.join("bin/win_x64/eurotrucks2.exe"))
            .filter(|p| p.is_file())
    }
    pub fn prefix(&self) -> PathBuf {
        // Steam keeps compatdata alongside the game library, including external drives.
        self.game
            .as_ref()
            .and_then(|g| g.parent())
            .and_then(|p| p.parent())
            .map(|p| p.join("compatdata/227300"))
            .unwrap_or_else(|| self.root.join("steamapps/compatdata/227300"))
    }
    pub fn ready(&self) -> bool {
        self.windows_game().is_some()
            && self.proton.is_some()
            && self.runtime.is_some()
            && self.configured
    }
}

pub fn process_running(name: &str) -> bool {
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        fs::read_to_string(entry.path().join("comm")).is_ok_and(|comm| comm.trim() == name)
    })
}

pub fn launcher_running() -> bool {
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        fs::read(entry.path().join("cmdline")).is_ok_and(|bytes| {
            let first = bytes.split(|b| *b == 0).next().unwrap_or_default();
            String::from_utf8_lossy(first)
                .to_ascii_lowercase()
                .ends_with("truckersmp-launcher.exe")
        })
    })
}

fn obj_mut<'a>(value: &'a mut Value<'static>, keys: &[&str]) -> Result<&'a mut Obj<'static>> {
    let Value::Obj(obj) = value else {
        bail!("Unexpected Steam config value; no changes made");
    };
    if keys.is_empty() {
        return Ok(obj);
    }
    let values = obj
        .entry(Cow::Owned(keys[0].to_string()))
        .or_insert_with(|| vec![Value::Obj(Obj::new())]);
    ensure!(
        values.len() == 1,
        "Ambiguous Steam config key; no changes made"
    );
    obj_mut(&mut values[0], &keys[1..])
}

pub fn configured_vdf(source: &str) -> Result<String> {
    let partial = keyvalues_parser::parse(source)?;
    ensure!(
        partial.bases.is_empty(),
        "Steam config uses external includes; refusing to rewrite"
    );
    let mut vdf = partial.into_vdf().into_owned();
    ensure!(
        vdf.key == "InstallConfigStore",
        "Unexpected Steam config root"
    );
    let mapping = obj_mut(
        &mut vdf.value,
        &["Software", "Valve", "Steam", "CompatToolMapping"],
    )?;
    let mut ets2 = Obj::new();
    for (key, value) in [
        ("name", "proton_experimental"),
        ("config", ""),
        ("priority", "250"),
    ] {
        ets2.insert(
            Cow::Owned(key.into()),
            vec![Value::Str(Cow::Owned(value.into()))],
        );
    }
    mapping.insert(Cow::Owned(ETS2.into()), vec![Value::Obj(ets2)]);
    Ok(vdf.to_string())
}

pub fn configure(steam: &Steam, data: &Path, progress: Progress<'_>) -> Result<()> {
    ensure!(
        !steam.game_running,
        "Close ETS2 before changing its Steam compatibility settings"
    );
    if steam.configured {
        progress("ETS2 already uses Proton Experimental".into());
    } else {
        if steam.running {
            progress("Closing Steam to safely update ETS2's compatibility setting…".into());
            Command::new("steam")
                .arg("-shutdown")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let started = Instant::now();
            while process_running("steam") && started.elapsed() < Duration::from_secs(30) {
                thread::sleep(Duration::from_millis(500));
            }
            ensure!(
                !process_running("steam"),
                "Steam did not close. Exit it normally and choose Prepare Steam again."
            );
        }
        let path = steam.root.join("config/config.vdf");
        let original = fs::read_to_string(&path)?;
        let changed = configured_vdf(&original)?;
        fs::create_dir_all(data.join("backups"))?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(data.join("backups"), fs::Permissions::from_mode(0o700))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let backup = data
            .join("backups")
            .join(format!("steam-config-{stamp}.vdf"));
        vendor::atomic_write(&backup, original.as_bytes())?;
        fs::set_permissions(&backup, fs::Permissions::from_mode(0o600))?;
        ensure!(
            !process_running("steam"),
            "Steam restarted during configuration; no changes made"
        );
        vendor::atomic_write(&path, changed.as_bytes())?;
        progress("Backed up Steam settings and enabled Proton Experimental for ETS2".into());
    }
    progress("Steam will download Windows ETS2 and its Proton dependencies. Complete any Steam prompts. Close ETS2 after its first launch, then return here.".into());
    Command::new("steam")
        .args(["-applaunch", ETS2])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Could not start native Steam")?;
    Ok(())
}

pub fn proton_command(steam: &Steam) -> Result<Command> {
    let mut command = Command::new(
        steam
            .runtime
            .as_ref()
            .context("Steam Linux Runtime 4 is not installed")?,
    );
    command
        .arg("--")
        .arg(
            steam
                .proton
                .as_ref()
                .context("Proton Experimental is not installed")?,
        )
        .arg("run")
        .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam.root)
        .env("STEAM_COMPAT_DATA_PATH", steam.prefix())
        .env("SteamAppId", ETS2)
        .env("SteamGameId", ETS2);
    Ok(command)
}

pub fn windows_path(path: &Path) -> Result<String> {
    ensure!(path.is_absolute(), "Expected an absolute path");
    let text = path.to_str().context("Path is not valid UTF-8")?;
    ensure!(
        !text.contains(['\n', '\r', '\0']),
        "Path contains unsupported characters"
    );
    Ok(format!("Z:{}", text.replace('/', "\\")))
}

/// Read the Windows executable's ProductVersion string without running the game.
pub fn game_version(path: &Path) -> Result<String> {
    ensure!(
        fs::metadata(path)?.len() <= 512 * 1024 * 1024,
        "Unexpectedly large game executable"
    );
    let bytes = fs::read(path)?;
    ensure!(
        bytes.starts_with(b"MZ"),
        "ETS2 executable is not a Windows PE file"
    );
    let key: Vec<u8> = "ProductVersion"
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    for (offset, _) in bytes
        .windows(key.len())
        .enumerate()
        .filter(|(_, w)| *w == key.as_slice())
    {
        let tail = &bytes[offset + key.len()..];
        let words: Vec<_> = tail
            .chunks_exact(2)
            .take(80)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let version: String = words
            .into_iter()
            .skip_while(|c| *c == 0)
            .take_while(|c| *c == b'.' as u16 || (*c >= b'0' as u16 && *c <= b'9' as u16))
            .filter_map(|c| char::from_u32(c as u32))
            .collect();
        if version.split('.').count() == 4
            && version
                .split('.')
                .all(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        {
            return Ok(version);
        }
    }
    bail!("Cannot read ETS2's Windows version. Verify its game files in Steam before launching.")
}
