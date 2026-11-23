//! Cài đặt dịch vụ `org.freedesktop.IBus.Engine` bằng Rust thuần sử dụng [`AdapterController`].

use std::path::PathBuf;
use std::time::SystemTime;

use vntype_adapter::{AdapterController, Effect, char_to_utf16_offset};
use vntype_config::{Config, ToneStyle, UnicodeForm, load_config_or_default, save_config_atomic};
use vntype_engine::{FieldKind, Key, OutputMode};

use crate::keymap::{IBUS_RELEASE_MASK, translate_ibus_key};

/// Tín hiệu đầu ra phát từ dịch vụ IBus Engine xuống IBus Daemon qua D-Bus.
#[derive(Debug, Clone, PartialEq)]
pub enum IbusSignal {
    /// Cập nhật chuỗi gạch chân (`UpdatePreeditText(text, cursor_utf16, visible)`).
    UpdatePreeditText {
        /// Nội dung văn bản gạch chân.
        text: String,
        /// Vị trí con trỏ theo đơn vị UTF-16.
        cursor_utf16: u32,
        /// Trạng thái hiển thị.
        visible: bool,
    },
    /// Ẩn vùng gạch chân (`HidePreeditText`).
    HidePreeditText,
    /// Chốt văn bản vào ứng dụng (`CommitText(text)`).
    CommitText {
        /// Chuỗi văn bản cần chốt.
        text: String,
    },
    /// Xoá văn bản xung quanh con trỏ (`DeleteSurroundingText(offset_chars, nchars)`).
    DeleteSurroundingText {
        /// Độ lệch điểm bắt đầu xoá (số nguyên âm tính từ con trỏ).
        offset_chars: i32,
        /// Số lượng ký tự Unicode (`char`) cần xoá.
        nchars: u32,
    },
    /// Chuyển tiếp sự kiện phím về cho ứng dụng (`ForwardKeyEvent(keyval, keycode, state)`).
    ForwardKeyEvent {
        /// Mã keyval của IBus.
        keyval: u32,
        /// Mã phím vật lý (keycode).
        keycode: u32,
        /// Cờ trạng thái modifier.
        state: u32,
    },
    /// Đăng ký toàn bộ menu thuộc tính (`RegisterProperties`) với IBus.
    RegisterMenu {
        /// Ảnh chụp cấu hình hiện tại.
        config: Box<Config>,
    },
    /// Cập nhật trạng thái menu thuộc tính (`UpdateProperty`) khi người dùng đổi cấu hình.
    UpdateMenu {
        /// Khoá thuộc tính vừa được kích hoạt.
        changed_key: String,
        /// Ảnh chụp cấu hình hiện tại.
        config: Box<Config>,
    },
}

/// Bộ xử lý giao thức IBus Engine bọc quanh [`AdapterController`].
pub struct IbusEngineService {
    controller: AdapterController,
    config_path: PathBuf,
    config_mtime: Option<SystemTime>,
    signals: Vec<IbusSignal>,
    holding_eaten_backspace: bool,
}

impl IbusEngineService {
    /// Khởi tạo dịch vụ IBus Engine với đường dẫn tệp cấu hình `config_path`.
    pub fn new(config_path: PathBuf) -> Self {
        let cfg = load_config_or_default(&config_path);
        let mtime = std::fs::metadata(&config_path).and_then(|m| m.modified()).ok();
        IbusEngineService {
            controller: AdapterController::new(cfg, OutputMode::Preedit),
            config_path,
            config_mtime: mtime,
            signals: Vec::new(),
            holding_eaten_backspace: false,
        }
    }

    /// Lấy và xoá danh sách các tín hiệu D-Bus vừa phát sinh.
    pub fn drain_signals(&mut self) -> Vec<IbusSignal> {
        std::mem::take(&mut self.signals)
    }

    /// Xử lý phương thức D-Bus `ProcessKeyEvent(keyval, keycode, state) -> bool`.
    ///
    /// Trả về `true` nếu phím đã được bộ gõ tiêu thụ, `false` nếu trả phím về cho ứng dụng.
    pub fn process_key_event(&mut self, keyval: u32, keycode: u32, state: u32) -> bool {
        if keyval == 0xff08 && (state & IBUS_RELEASE_MASK) != 0 {
            self.holding_eaten_backspace = false;
            return false;
        }
        let Some(ev) = translate_ibus_key(keyval, state) else {
            return false;
        };
        if ev.alt && !ev.ctrl && !ev.meta && ev.key == Key::Char(',') {
            self.holding_eaten_backspace = false;
            return false;
        }
        self.reload_config_if_needed();
        let effects = self.controller.process_key(ev, keyval);
        let handled = self.apply_effects(effects, keyval, keycode, state);
        if keyval == 0xff08 {
            if handled {
                self.holding_eaten_backspace = true;
            }
        } else {
            self.holding_eaten_backspace = false;
        }
        handled
    }

    /// Xử lý tín hiệu `SetSurroundingText(text, cursor_chars, anchor_chars)` từ IBus.
    pub fn set_surrounding_text(&mut self, text: &str, cursor_chars: u32, anchor_chars: u32) {
        let Some(c_u16) = char_to_utf16_offset(text, cursor_chars) else {
            self.controller.set_surrounding_utf16(None, 0, 0);
            return;
        };
        let Some(a_u16) = char_to_utf16_offset(text, anchor_chars) else {
            self.controller.set_surrounding_utf16(None, 0, 0);
            return;
        };
        self.controller.set_surrounding_utf16(Some(text), c_u16, a_u16);
    }

    /// Xử lý tín hiệu `SetContentType(purpose, hints)` từ IBus.
    pub fn set_content_type(&mut self, purpose: u32, _hints: u32) {
        let kind = match purpose {
            2..=4 | 9 => FieldKind::Number,
            5 => FieldKind::Url,
            6 => FieldKind::Email,
            8 => FieldKind::Password,
            _ => FieldKind::Text,
        };
        let effects = self.controller.set_field_kind(kind);
        self.apply_effects(effects, 0, 0, 0);
    }

    /// Trả về kiểu gõ hiện tại đang cấu hình trong engine.
    pub fn input_method(&self) -> String {
        self.controller.session().config().input_method.to_ascii_lowercase()
    }

    /// Trả về bản sao cấu hình hiện tại trong engine.
    pub fn current_config(&self) -> Config {
        self.controller.session().config().clone()
    }

    /// Phát tín hiệu đăng ký menu thuộc tính IBus (`RegisterProperties`).
    pub fn emit_register_menu(&mut self) {
        self.signals.push(IbusSignal::RegisterMenu { config: Box::new(self.current_config()) });
    }

    /// Xử lý sự kiện người dùng chọn một mục trong menu IBus (`PropertyActivate(name, state)`).
    pub fn property_activate(&mut self, name: &str, state: u32) {
        let mut cfg = self.current_config();
        let checked = state != 0;
        let mut changed = true;

        match name {
            "vntype.im.telex" if checked => cfg.input_method = "telex".into(),
            "vntype.im.vni" if checked => cfg.input_method = "vni".into(),
            "vntype.im.simple_telex" if checked => cfg.input_method = "simple_telex".into(),
            "vntype.im.viqr" if checked => cfg.input_method = "viqr".into(),
            _ if name.starts_with("vntype.charset.") && checked => {
                let key = &name["vntype.charset.".len()..];
                if matches!(
                    key,
                    "unicode"
                        | "tcvn3"
                        | "vni_windows"
                        | "viqr"
                        | "cp1258"
                        | "unicode_nfd"
                        | "utf8_literal"
                        | "ncr_decimal"
                        | "ncr_hex"
                        | "cstring_hex"
                        | "x_utf8"
                        | "viscii"
                        | "vps"
                        | "bkhcm2"
                        | "bkhcm1"
                        | "vietware_x"
                        | "vietware_full"
                ) {
                    cfg.charset = key.into();
                    cfg.unicode_form =
                        if key == "unicode_nfd" { UnicodeForm::Nfd } else { UnicodeForm::Nfc };
                } else {
                    changed = false;
                }
            }
            "vntype.tone.new" if checked => cfg.tone_style = ToneStyle::New,
            "vntype.tone.old" if checked => cfg.tone_style = ToneStyle::Old,
            "vntype.opt.spell_check" => cfg.spell_check = checked,
            "vntype.opt.w_as_u" => cfg.w_as_u = checked,
            "vntype.opt.bracket_keys" => cfg.bracket_keys = checked,
            "vntype.opt.quick_telex" => cfg.quick_telex = checked,
            "vntype.opt.allow_coda_k" => cfg.allow_coda_k = checked,
            "vntype.opt.edit_previous_word" => cfg.edit_previous_word = checked,
            "vntype.opt.restore_after_space" => cfg.restore_after_space = checked,
            "vntype.opt.auto_capitalize" => cfg.auto_capitalize = checked,
            "vntype.opt.smart_caps" => cfg.smart_caps = checked,
            "vntype.macro.enabled" => cfg.macro_enabled = checked,
            "vntype.macro.undo" => cfg.macro_undo = checked,
            "vntype.macro.digits" => cfg.macro_digits = checked,
            "vntype.macro.in_off_mode" => cfg.macro_in_off_mode = checked,
            _ => changed = false,
        }

        if !changed {
            return;
        }

        let _ = save_config_atomic(&self.config_path, &cfg);
        if let Ok(json) = serde_json::to_string(&cfg)
            && let Some(effects) = self.controller.apply_config_json(&json)
        {
            self.apply_effects(effects, 0, 0, 0);
        }
        self.config_mtime = std::fs::metadata(&self.config_path).and_then(|m| m.modified()).ok();
        self.signals.push(IbusSignal::UpdateMenu { changed_key: name.to_string(), config: Box::new(cfg) });
    }

    /// Xử lý sự kiện nhận tiêu điểm `FocusIn(app_id)`.
    pub fn focus_in(&mut self, app_id: Option<&str>) {
        self.holding_eaten_backspace = false;
        let effects = self.controller.focus_changed(app_id);
        self.apply_effects(effects, 0, 0, 0);
        self.emit_register_menu();
    }

    /// Xử lý sự kiện mất tiêu điểm `FocusOut()`.
    pub fn focus_out(&mut self) {
        self.holding_eaten_backspace = false;
        let effects = self.controller.flush();
        self.apply_effects(effects, 0, 0, 0);
    }

    /// Xử lý sự kiện `Reset()` từ IBus.
    ///
    /// Lưu ý: IBus gửi `Reset` ngay sau khi phím Backspace xoá dấu cách. Nếu phiên vừa khôi phục
    /// từ đã chốt (`is_adopted()`), ta chỉ ẩn vùng gạch chân mà không xoá trạng thái từ đó.
    pub fn reset(&mut self) {
        if self.controller.session().is_adopted() {
            self.signals.push(IbusSignal::HidePreeditText);
            return;
        }
        self.controller.reset();
        self.signals.push(IbusSignal::HidePreeditText);
    }

    fn apply_effects(&mut self, effects: Vec<Effect>, keyval: u32, keycode: u32, state: u32) -> bool {
        let mut forward_original = false;
        for eff in effects {
            match eff {
                Effect::SetPreedit { text, cursor_utf16, .. } => {
                    self.signals.push(IbusSignal::UpdatePreeditText { text, cursor_utf16, visible: true });
                }
                Effect::ClearPreedit => {
                    self.signals.push(IbusSignal::HidePreeditText);
                }
                Effect::CommitText { text } => {
                    self.signals.push(IbusSignal::CommitText { text });
                }
                Effect::ReplaceText { delete_chars, insert, .. } => {
                    if delete_chars > 0 {
                        self.signals.push(IbusSignal::DeleteSurroundingText {
                            offset_chars: -(delete_chars as i32),
                            nchars: delete_chars,
                        });
                    }
                    if !insert.is_empty() {
                        self.signals.push(IbusSignal::CommitText { text: insert });
                    }
                }
                Effect::WaitMs { .. } => {}
                Effect::ForwardKey { raw_keycode } => {
                    let _ = raw_keycode;
                    forward_original = true;
                }
            }
        }
        if forward_original && keyval == 0xff08 && self.holding_eaten_backspace && keycode != 0 {
            self.signals.push(IbusSignal::ForwardKeyEvent { keyval, keycode, state });
            self.signals.push(IbusSignal::ForwardKeyEvent {
                keyval,
                keycode,
                state: state | IBUS_RELEASE_MASK,
            });
            return true;
        }
        !forward_original
    }

    fn reload_config_if_needed(&mut self) {
        if !self.controller.display_text().is_empty() {
            return;
        }
        let mtime = std::fs::metadata(&self.config_path).and_then(|m| m.modified()).ok();
        if mtime != self.config_mtime {
            self.config_mtime = mtime;
            if let Ok(json) = std::fs::read_to_string(&self.config_path)
                && let Some(effects) = self.controller.apply_config_json(&json)
            {
                self.apply_effects(effects, 0, 0, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ibus_rust_service_types_vietnamese() {
        let mut svc = IbusEngineService::new(PathBuf::from("/nonexistent/config.json"));
        for b in b"tieengs" {
            assert!(svc.process_key_event(*b as u32, 0, 0));
        }
        let signals = svc.drain_signals();
        assert!(
            signals
                .iter()
                .any(|s| matches!(s, IbusSignal::UpdatePreeditText { text, .. } if text == "tiếng"))
        );

        assert!(svc.process_key_event(b' ' as u32, 0, 0));
        let commit_signals = svc.drain_signals();
        assert!(
            commit_signals.iter().any(|s| matches!(s, IbusSignal::CommitText { text } if text == "tiếng "))
        );
    }

    #[test]
    fn ibus_surrounding_password_and_focus() {
        let mut svc = IbusEngineService::new(PathBuf::from("/nonexistent/config.json"));
        svc.focus_in(Some("chrome"));
        svc.set_surrounding_text("tiêng", 5, 5);
        assert!(svc.process_key_event(b's' as u32, 0, 0));
        let sigs = svc.drain_signals();
        assert!(sigs.iter().any(|s| matches!(s, IbusSignal::DeleteSurroundingText { nchars: 3, .. })));
        assert!(sigs.iter().any(|s| matches!(s, IbusSignal::CommitText { text } if text == "ếng")));

        // Reset khi đang adopted chỉ ẩn preedit, không xoá trạng thái từ đang sửa
        svc.reset();
        assert_eq!(svc.drain_signals(), vec![IbusSignal::HidePreeditText]);

        // Chuyển sang ô mật khẩu -> pass phím qua
        svc.set_content_type(8, 0);
        let _ = svc.drain_signals();
        assert!(!svc.process_key_event(b'a' as u32, 30, 0));

        svc.focus_out();
    }

    #[test]
    fn held_backspace_forwards_synthetic_repeats_after_word_cleared() {
        let mut svc = IbusEngineService::new(PathBuf::from("/nonexistent/config.json"));
        for b in b"ba" {
            assert!(svc.process_key_event(*b as u32, 0, 0));
        }
        let _ = svc.drain_signals();

        // Xoá hết từ đang gõ bằng giữ phím Backspace (keycode = 14)
        assert!(svc.process_key_event(0xff08, 14, 0));
        assert!(svc.process_key_event(0xff08, 14, 0));
        let _ = svc.drain_signals();

        // Lần lặp tiếp theo khi từ đã trống phải phát ForwardKeyEvent để tránh kẹt auto-repeat
        assert!(svc.process_key_event(0xff08, 14, 0));
        let sigs = svc.drain_signals();
        assert_eq!(
            sigs,
            vec![
                IbusSignal::ForwardKeyEvent { keyval: 0xff08, keycode: 14, state: 0 },
                IbusSignal::ForwardKeyEvent { keyval: 0xff08, keycode: 14, state: IBUS_RELEASE_MASK },
            ]
        );

        // Nhả phím Backspace -> lần nhấn mới trả về false để app tự xử lý bình thường
        assert!(!svc.process_key_event(0xff08, 14, IBUS_RELEASE_MASK));
        assert!(!svc.process_key_event(0xff08, 14, 0));
    }

    #[test]
    fn property_activate_updates_method_charset_options_and_ignores_alt_comma() {
        let tmp = std::env::temp_dir().join(format!("vntype_ibus_prop_{}.json", std::process::id()));
        let mut svc = IbusEngineService::new(tmp.clone());

        // Alt+, không còn bật/tắt chế độ trong vntype-ibus mà đi thẳng qua ứng dụng
        assert!(!svc.process_key_event(b',' as u32, 0, crate::keymap::IBUS_MOD1_MASK));
        assert!(svc.drain_signals().is_empty());

        svc.property_activate("vntype.im.vni", 1);
        svc.property_activate("vntype.charset.tcvn3", 1);
        svc.property_activate("vntype.tone.old", 1);
        svc.property_activate("vntype.opt.w_as_u", 1);
        svc.property_activate("vntype.macro.enabled", 0);

        let cfg = svc.current_config();
        assert_eq!(cfg.input_method, "vni");
        assert_eq!(cfg.charset, "tcvn3");
        assert_eq!(cfg.unicode_form, UnicodeForm::Nfc);
        assert_eq!(cfg.tone_style, ToneStyle::Old);
        assert!(cfg.w_as_u);
        assert!(!cfg.macro_enabled);

        svc.property_activate("vntype.charset.unicode_nfd", 1);
        assert_eq!(svc.current_config().unicode_form, UnicodeForm::Nfd);
        assert!(svc.drain_signals().iter().any(|s| matches!(s, IbusSignal::UpdateMenu { .. })));

        let _ = std::fs::remove_file(tmp);
    }
}
