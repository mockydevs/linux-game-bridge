use linux_game_bridge::steam::{Steam, configured_vdf, library_paths, windows_path};
use std::path::Path;

#[test]
fn changes_only_ets2_mapping_preserving_other_games_and_settings() {
    let source = r#""InstallConfigStore" { "Software" { "Valve" { "Steam" {
      "UnrelatedSetting" "keep-me" "CompatToolMapping" {
        "0" { "name" "proton-stable" } "42" { "name" "custom-tool" }
        "227300" { "name" "old-tool" "config" "old" }
      }
    } } } "Other" { "Nested" "also-keep" } }"#;
    let result = configured_vdf(source).unwrap();
    assert_eq!(
        configured_vdf(&result).unwrap(),
        result,
        "configuration should be idempotent"
    );
    let parsed = keyvalues_parser::parse(&result).unwrap();
    let root = parsed.value.get_obj().unwrap();
    assert_eq!(
        root["Other"][0].get_obj().unwrap()["Nested"][0]
            .get_str()
            .unwrap(),
        "also-keep"
    );
    let steam = &root["Software"][0].get_obj().unwrap()["Valve"][0]
        .get_obj()
        .unwrap()["Steam"][0];
    let obj = steam.get_obj().unwrap();
    assert_eq!(obj["UnrelatedSetting"][0].get_str().unwrap(), "keep-me");
    let mapping = obj["CompatToolMapping"][0].get_obj().unwrap();
    for (id, expected) in [
        ("0", "proton-stable"),
        ("42", "custom-tool"),
        ("227300", "proton_experimental"),
    ] {
        assert_eq!(
            mapping[id][0].get_obj().unwrap()["name"][0]
                .get_str()
                .unwrap(),
            expected
        );
    }
}
#[test]
fn discovers_external_libraries_with_spaces_and_deduplicates_primary() {
    let libs = library_paths(
        r#""libraryfolders" { "0" { "path" "/primary" } "1" { "path" "/media/My Games" } }"#,
        Path::new("/primary"),
    )
    .unwrap();
    assert_eq!(
        libs,
        vec![Path::new("/primary"), Path::new("/media/My Games")]
    );
}
#[test]
fn refuses_ambiguous_or_unexpected_steam_config() {
    assert!(configured_vdf(r#""wrong-root" {}"#).is_err());
    assert!(configured_vdf(r#""InstallConfigStore" { "Software" "not-an-object" }"#).is_err());
    assert!(configured_vdf(r#""InstallConfigStore" { "Software" {} "Software" {} }"#).is_err());
}
#[test]
fn compatibility_data_follows_game_library_not_primary_steam_drive() {
    let steam = Steam {
        root: "/primary".into(),
        libraries: vec![],
        game: Some("/external/steamapps/common/Euro Truck Simulator 2".into()),
        proton: None,
        runtime: None,
        running: false,
        game_running: false,
        configured: false,
    };
    assert_eq!(
        steam.prefix(),
        Path::new("/external/steamapps/compatdata/227300")
    );
}
#[test]
fn windows_path_preserves_spaces_and_rejects_relative_or_control_characters() {
    assert_eq!(
        windows_path(Path::new("/media/My Games/ETS2")).unwrap(),
        "Z:\\media\\My Games\\ETS2"
    );
    assert!(windows_path(Path::new("relative/path")).is_err());
    assert!(windows_path(Path::new("/path\nnew-line")).is_err());
}
#[test]
fn reads_game_product_version_and_rejects_unsupported_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("game.exe");
    let mut bytes = b"MZ-resource".to_vec();
    for word in "ProductVersion\0\0"
        .encode_utf16()
        .chain("1.61.1.9\0".encode_utf16())
    {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        linux_game_bridge::steam::game_version(&path).unwrap(),
        "1.61.1.9"
    );
    std::fs::write(&path, b"not an executable").unwrap();
    assert!(linux_game_bridge::steam::game_version(&path).is_err());
}
