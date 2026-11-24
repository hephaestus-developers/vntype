//! Integration tests kiểm tra luồng điều phối đầy đủ của `vntype-adapter`.

use vntype_adapter::{AdapterController, Effect};
use vntype_engine::{Config, KeyEvent, OutputMode};

#[test]
fn end_to_end_surrogate_surrounding_hotkey_and_config_patch() {
    let mut ctrl = AdapterController::new(Config::default(), OutputMode::Replace);
    ctrl.set_surrounding_utf16(Some("😀 tiêng"), 8, 8);
    let effects = ctrl.process_key(KeyEvent::ch('s'), 31);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::ReplaceText { delete_utf16: 3, delete_chars: 3, insert, .. } if insert == "ếng"
    )));

    // Đổi cấu hình sang VNI + switch_key Alt+, qua JSON patch và kiểm tra Alt+, tăng config_version
    assert!(ctrl.apply_config_json(r#"{"input_method": "vni", "switch_key": "alt_comma"}"#).is_some());
    ctrl.focus_changed(Some("chrome"));
    let v0 = ctrl.config_version();
    let mut alt_comma = KeyEvent::ch(',');
    alt_comma.alt = true;
    ctrl.process_key(alt_comma, 51);
    assert!(!ctrl.is_enabled());
    assert_eq!(ctrl.config_version(), v0 + 1);
}
