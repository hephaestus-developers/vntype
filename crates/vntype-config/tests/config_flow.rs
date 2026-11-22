//! Integration tests kiểm tra luồng cấu hình đầu-cuối của `vntype-config`.

use std::sync::atomic::{AtomicU64, Ordering};
use vntype_config::{
    AppProfile, Config, OutputMode, ToneStyle, fingerprint_bytes, load_config_file, normalize_app_id,
    resolve_profile, save_config_atomic,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

#[test]
fn end_to_end_save_patch_fingerprint_and_resolve() {
    let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("vntype_cfg_it_{}_{}", std::process::id(), seq));
    let path = dir.join("nested").join("config.json");

    let mut cfg = Config::default();
    cfg.app_profiles.insert(
        normalize_app_id(Some("alacritty")),
        AppProfile { vietnamese: Some(false), ..Default::default() },
    );

    save_config_atomic(&path, &cfg).unwrap();
    let raw_bytes = std::fs::read(&path).unwrap();
    let loaded = load_config_file(&path).unwrap();
    assert_eq!(loaded, cfg);

    let patched = loaded.apply_json_patch(r#"{"tone_style": "old"}"#).unwrap();
    assert_eq!(patched.tone_style, ToneStyle::Old);
    save_config_atomic(&path, &patched).unwrap();

    let new_bytes = std::fs::read(&path).unwrap();
    assert_ne!(fingerprint_bytes(&raw_bytes), fingerprint_bytes(&new_bytes));

    let profile = resolve_profile(&patched, Some("io.alacritty"));
    assert_eq!(profile.output_mode, Some(OutputMode::Replace));
    assert_eq!(profile.vietnamese, Some(false));

    let _ = std::fs::remove_dir_all(&dir);
}
