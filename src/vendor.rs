use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};

pub const INSTALLER: &str =
    "https://files.launcher.truckersmp.com/truckersmp-launcher/win/x64/TruckersMP-Setup.exe";
const MANIFEST: &str = "https://update.ets2mp.com/files.json";
const DOWNLOAD: &str = "https://download-new.ets2mp.com/files";
const MAX_FILE: u64 = 512 * 1024 * 1024;
pub type Progress<'a> = &'a dyn Fn(String);

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(300))
        .connect_timeout(Duration::from_secs(20))
        .user_agent("Linux Game Bridge/0.1 (unofficial Linux assistant)")
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 || attempt.url().scheme() != "https" {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

fn get(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .header("Cache-Control", "no-cache")
        .send()?
        .error_for_status()?;
    ensure!(
        response.content_length().unwrap_or(0) <= limit,
        "Download exceeds size limit"
    );
    let mut bytes = Vec::new();
    response.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Download exceeds size limit");
    Ok(bytes)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Destination has no parent")?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn safe_relative(name: &str) -> Result<PathBuf> {
    ensure!(
        !name.is_empty() && !name.contains(['\\', ':', '\0']),
        "Unsafe file path"
    );
    let path = Path::new(name);
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "Unsafe file path: {name}"
    );
    ensure!(!path.as_os_str().is_empty(), "Empty file path");
    Ok(path.to_path_buf())
}

fn secure_destination(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    ensure!(
        !fs::symlink_metadata(root)?.file_type().is_symlink(),
        "Installation root cannot be a symlink"
    );
    for component in relative.components() {
        path.push(component);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            ensure!(
                !meta.file_type().is_symlink(),
                "Refusing symlink inside installation: {}",
                path.display()
            );
        }
    }
    Ok(path)
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(rename = "Files")]
    files: Vec<ModFile>,
}
#[derive(Deserialize)]
struct ModFile {
    #[serde(rename = "FilePath")]
    path: String,
    #[serde(rename = "Md5")]
    md5: String,
    #[serde(rename = "Type")]
    kind: String,
}
#[derive(Debug, Deserialize)]
pub struct Version {
    pub name: String,
    pub supported_game_version: String,
    pub ets2mp_checksum: CoreChecksum,
}
#[derive(Debug, Deserialize)]
pub struct CoreChecksum {
    pub dll: String,
}

pub fn version(client: &Client) -> Result<Version> {
    Ok(serde_json::from_slice(&get(
        client,
        "https://api.truckersmp.com/v2/version",
        1024 * 1024,
    )?)?)
}

fn md5_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut context = md5::Context::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        context.consume(&buffer[..n]);
    }
    Ok(format!("{:x}", context.finalize()))
}

pub fn update_mod(client: &Client, root: &Path, progress: Progress<'_>) -> Result<Version> {
    fs::create_dir_all(root)?;
    let manifest: Manifest = serde_json::from_slice(&get(client, MANIFEST, 4 * 1024 * 1024)?)?;
    let files: Vec<_> = manifest
        .files
        .into_iter()
        .filter(|f| f.kind == "ets2" || f.kind == "system")
        .collect();
    ensure!(
        !files.is_empty() && files.iter().any(|f| f.path == "/core_ets2mp.dll"),
        "Incomplete official manifest"
    );
    // Validate the entire manifest before changing any installed file.
    for f in &files {
        safe_relative(f.path.strip_prefix('/').context("Invalid manifest path")?)?;
        ensure!(valid_md5(&f.md5), "Invalid checksum in manifest");
    }
    let mut changed = 0;
    for (index, entry) in files.iter().enumerate() {
        let rel = safe_relative(entry.path.strip_prefix('/').unwrap())?;
        let dst = secure_destination(root, &rel)?;
        if md5_file(&dst).ok().as_deref() == Some(entry.md5.as_str()) {
            continue;
        }
        progress(format!(
            "Updating {} ({}/{})",
            rel.display(),
            index + 1,
            files.len()
        ));
        let url = format!("{DOWNLOAD}{}", entry.path);
        let bytes = get(client, &url, MAX_FILE)?;
        write_verified(&dst, &bytes, &entry.md5)?;
        changed += 1;
    }
    let current = version(client)?;
    ensure!(
        md5_file(&root.join("core_ets2mp.dll"))? == current.ets2mp_checksum.dll,
        "TruckersMP changed during update. Try again before launching."
    );
    progress(format!(
        "Verified {} files; updated {changed}. TruckersMP {}",
        files.len(),
        current.name
    ));
    Ok(current)
}
fn write_verified(path: &Path, bytes: &[u8], expected: &str) -> Result<()> {
    ensure!(
        format!("{:x}", md5::compute(bytes)).eq_ignore_ascii_case(expected),
        "Checksum mismatch for {}; launch stopped",
        path.display()
    );
    atomic_write(path, bytes)
}

fn valid_md5(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

// Squirrel embeds a ZIP inside PE resources and may append an Authenticode signature.
// Locate the ZIP by its end record, not by trusting a hardcoded PE resource offset.
pub fn embedded_zip(bytes: &[u8]) -> Result<&[u8]> {
    for end in (0..bytes.len().saturating_sub(21)).rev() {
        if &bytes[end..end + 4] != b"PK\x05\x06" {
            continue;
        }
        let u16_at = |offset| {
            u16::from_le_bytes(bytes[end + offset..end + offset + 2].try_into().unwrap()) as usize
        };
        let u32_at = |offset| {
            u32::from_le_bytes(bytes[end + offset..end + offset + 4].try_into().unwrap()) as usize
        };
        let archive_end = end + 22 + u16_at(20);
        if archive_end > bytes.len() {
            continue;
        }
        let Some(start) = end
            .checked_sub(u32_at(12))
            .and_then(|n| n.checked_sub(u32_at(16)))
        else {
            continue;
        };
        if bytes.get(start..start + 4) == Some(b"PK\x03\x04") {
            let candidate = &bytes[start..archive_end];
            if zip::ZipArchive::new(Cursor::new(candidate)).is_ok() {
                return Ok(candidate);
            }
        }
    }
    bail!(
        "Official installer format changed: no supported Squirrel ZIP found. No installer was executed."
    )
}

fn unpack_package(package: &[u8], dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(Cursor::new(package))?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let entry_name = entry.name()?.into_owned();
        let Some(name) = entry_name.strip_prefix("lib/net45/") else {
            continue;
        };
        if name.is_empty() || entry.is_dir() {
            continue;
        }
        let relative = safe_relative(name)?;
        ensure!(!entry.is_symlink(), "Archive contains a symlink");
        total = total
            .checked_add(entry.size())
            .context("Archive size overflow")?;
        ensure!(
            entry.size() <= MAX_FILE && total <= 1024 * 1024 * 1024,
            "Archive extraction limit exceeded"
        );
        let path = secure_destination(dest, &relative)?;
        fs::create_dir_all(path.parent().unwrap())?;
        let mut file = File::create(path)?;
        let copied = std::io::copy(&mut entry.by_ref().take(MAX_FILE + 1), &mut file)?;
        ensure!(copied <= MAX_FILE, "Archive entry exceeded limit");
    }
    ensure!(
        dest.join("TruckersMP-Launcher.exe").is_file(),
        "Official package has no launcher"
    );
    Ok(())
}

pub fn install_launcher(
    client: &Client,
    root: &Path,
    installer_override: Option<&Path>,
    progress: Progress<'_>,
) -> Result<PathBuf> {
    fs::create_dir_all(root)?;
    progress("Obtaining the official TruckersMP installer…".into());
    let bytes = if let Some(path) = installer_override {
        ensure!(fs::metadata(path)?.len() <= MAX_FILE, "Installer too large");
        fs::read(path)?
    } else {
        get(client, INSTALLER, MAX_FILE)?
    };
    ensure!(
        bytes.starts_with(b"MZ"),
        "Download is not a Windows installer"
    );
    let mut archive = zip::ZipArchive::new(Cursor::new(embedded_zip(&bytes)?))?;
    let mut releases = String::new();
    archive
        .by_name("RELEASES")?
        .take(1024 * 1024)
        .read_to_string(&mut releases)?;
    let fields: Vec<_> = releases
        .trim_start_matches('\u{feff}')
        .lines()
        .rfind(|l| l.contains("-full.nupkg"))
        .context("No full launcher release")?
        .split_whitespace()
        .collect();
    ensure!(fields.len() == 3, "Unexpected Squirrel release format");
    let package_name = fields[1];
    safe_relative(package_name)?;
    let version = package_name
        .strip_prefix("TruckersMP-")
        .and_then(|n| n.strip_suffix("-full.nupkg"))
        .context("Unexpected launcher package")?;
    ensure!(
        !version.is_empty()
            && version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
        "Invalid launcher version"
    );
    let mut package = Vec::new();
    archive
        .by_name(package_name)?
        .take(MAX_FILE + 1)
        .read_to_end(&mut package)?;
    ensure!(
        package.len() as u64 <= MAX_FILE && package.len() as u64 == fields[2].parse::<u64>()?,
        "Launcher package size mismatch"
    );
    ensure!(
        format!("{:x}", Sha1::digest(&package)).eq_ignore_ascii_case(fields[0]),
        "Launcher package checksum mismatch"
    );
    let stage = tempfile::tempdir_in(root)?;
    unpack_package(&package, stage.path())?;
    let target = secure_destination(root, &safe_relative(&format!("app-{version}"))?)?;
    if !target.exists() {
        fs::rename(stage.path(), &target)?;
    } else {
        repair_tree(stage.path(), &target)?;
    }
    let mut updater = Vec::new();
    archive
        .by_name("Update.exe")?
        .take(32 * 1024 * 1024)
        .read_to_end(&mut updater)?;
    ensure!(updater.starts_with(b"MZ"), "Invalid official updater");
    atomic_write(&root.join("Update.exe"), &updater)?;
    atomic_write(&root.join("packages/RELEASES"), releases.as_bytes())?;
    atomic_write(&root.join("packages").join(package_name), &package)?;
    atomic_write(
        &root.join("launcher.json"),
        &serde_json::to_vec(&format!("app-{version}"))?,
    )?;
    progress(format!("Official launcher {version} installed"));
    Ok(target)
}

fn repair_tree(source: &Path, target: &Path) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let dest = secure_destination(target, Path::new(&entry.file_name()))?;
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&dest)?;
            repair_tree(&entry.path(), &dest)?;
        } else {
            let bytes = fs::read(entry.path())?;
            if md5_file(&dest).ok().as_deref()
                != Some(format!("{:x}", md5::compute(&bytes)).as_str())
            {
                atomic_write(&dest, &bytes)?;
            }
        }
    }
    Ok(())
}

pub fn launcher(root: &Path) -> Result<PathBuf> {
    let name: String = serde_json::from_slice(&fs::read(root.join("launcher.json"))?)?;
    let mut name = name;
    let mut newest = Vec::<u64>::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().into_owned();
        let Some(version) = filename.strip_prefix("app-") else {
            continue;
        };
        let Ok(parts) = version
            .split('.')
            .map(str::parse::<u64>)
            .collect::<std::result::Result<Vec<_>, _>>()
        else {
            continue;
        };
        if entry.file_type()?.is_dir()
            && !parts.is_empty()
            && parts > newest
            && entry.path().join("TruckersMP-Launcher.exe").is_file()
        {
            newest = parts;
            name = filename;
        }
    }
    let rel = safe_relative(&name)?;
    let path = secure_destination(root, &rel)?;
    secure_destination(root, &rel.join("TruckersMP-Launcher.exe"))?;
    ensure!(
        path.join("TruckersMP-Launcher.exe").is_file(),
        "Official launcher is missing; run Install"
    );
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn extracts_resource_zip_despite_pe_prefix_and_signature_suffix() {
        let archive = zip_bytes(&[("RELEASES", b"release-data")]);
        let mut pe = b"MZ-pe-resource-header".to_vec();
        pe.extend_from_slice(&archive);
        pe.extend_from_slice(b"authenticode-trailer");
        assert_eq!(embedded_zip(&pe).unwrap(), archive);
    }
    #[test]
    fn unsupported_and_truncated_installer_returns_error_without_panicking() {
        for data in [
            b"".as_slice(),
            b"PK\x05\x06",
            &[0; 40],
            b"MZ not an installer",
        ] {
            assert!(embedded_zip(data).is_err());
        }
    }
    #[test]
    fn rejects_paths_that_could_escape_installation() {
        for path in [
            "",
            "../outside",
            "/absolute",
            "a/../../outside",
            "C:drive",
            "a\\..\\outside",
            "a\0b",
        ] {
            assert!(safe_relative(path).is_err(), "accepted {path:?}");
        }
        assert_eq!(
            safe_relative("data/shared/mods/data2.mp").unwrap(),
            PathBuf::from("data/shared/mods/data2.mp")
        );
    }
    #[test]
    fn corrupt_download_preserves_previous_working_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("core.dll");
        fs::write(&path, b"working-version").unwrap();
        let expected = format!("{:x}", md5::compute(b"new-version"));
        assert!(write_verified(&path, b"truncated-response", &expected).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"working-version");
        write_verified(&path, b"new-version", &expected).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new-version");
    }
    #[test]
    fn archive_cannot_write_outside_staging_directory() {
        let temp = tempfile::tempdir().unwrap();
        let stage = temp.path().join("stage");
        fs::create_dir(&stage).unwrap();
        let archive = zip_bytes(&[("lib/net45/../../escape", b"bad")]);
        assert!(unpack_package(&archive, &stage).is_err());
        assert!(!temp.path().join("escape").exists());
    }
    #[test]
    fn refuses_symlinks_inside_installation() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), temp.path().join("data")).unwrap();
        assert!(secure_destination(temp.path(), Path::new("data/escape")).is_err());
    }
    #[test]
    fn repair_replaces_corrupt_launcher_resources() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("resources")).unwrap();
        fs::write(source.path().join("resources/app.asar"), b"correct").unwrap();
        fs::create_dir(target.path().join("resources")).unwrap();
        fs::write(target.path().join("resources/app.asar"), b"broken").unwrap();
        repair_tree(source.path(), target.path()).unwrap();
        assert_eq!(
            fs::read(target.path().join("resources/app.asar")).unwrap(),
            b"correct"
        );
    }
}

#[cfg(test)]
mod installer_tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn installs_squirrel_package_with_bom_release_manifest_and_repairs_resources() {
        let package = archive(&[
            ("lib/net45/TruckersMP-Launcher.exe", b"MZ-fake-test-exe"),
            ("lib/net45/resources/app.asar", b"test-resource"),
        ]);
        let releases = format!(
            "\u{feff}{:X} TruckersMP-1.3.19-full.nupkg {}",
            Sha1::digest(&package),
            package.len()
        );
        let zip = archive(&[
            ("RELEASES", releases.as_bytes()),
            ("TruckersMP-1.3.19-full.nupkg", &package),
            ("Update.exe", b"MZ-fake-updater"),
        ]);
        let mut pe = b"MZ-fake-resource-prefix".to_vec();
        pe.extend(zip);
        pe.extend(b"trailing-signature");
        let temp = tempfile::tempdir().unwrap();
        let installer = temp.path().join("setup.exe");
        fs::write(&installer, pe).unwrap();
        let root = temp.path().join("installed");
        let client = client().unwrap();
        let installed = install_launcher(&client, &root, Some(&installer), &|_| {}).unwrap();
        assert_eq!(
            fs::read(installed.join("resources/app.asar")).unwrap(),
            b"test-resource"
        );
        fs::write(installed.join("resources/app.asar"), b"corrupt").unwrap();
        install_launcher(&client, &root, Some(&installer), &|_| {}).unwrap();
        assert_eq!(
            fs::read(installed.join("resources/app.asar")).unwrap(),
            b"test-resource"
        );
        assert_eq!(launcher(&root).unwrap(), installed);
    }
}
