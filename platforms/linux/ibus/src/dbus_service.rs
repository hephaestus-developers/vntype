//! Cài đặt dịch vụ `org.freedesktop.IBus.Engine` bằng Rust thuần sử dụng [`AdapterController`].

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime};

use vntype_adapter::{AdapterController, Effect, char_to_utf16_offset};
use vntype_config::{
    Config, ToneStyle, UnicodeForm, ensure_macros_file, load_config_or_default, load_macros_file,
    save_config_atomic,
};
use vntype_engine::{FieldKind, Key, OutputMode};

use crate::keymap::{IBUS_RELEASE_MASK, IBUS_SYSTEM_MODIFIER_MASK, translate_ibus_key};

#[cfg(all(debug_assertions, not(test)))]
fn write_debug_log(msg: &str) {
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LOG_FILE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
    let lock = LOG_FILE.get_or_init(|| {
        Mutex::new(
            std::fs::OpenOptions::new().create(true).append(true).open("/tmp/vntype-ibus-debug.log").ok(),
        )
    });
    if let Ok(mut guard) = lock.lock()
        && let Some(f) = guard.as_mut()
    {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let _ = writeln!(f, "[{}.{:03}] {msg}", elapsed.as_secs(), elapsed.subsec_millis());
    }
}

macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(all(debug_assertions, not(test)))]
        write_debug_log(&format!($($arg)*));
    };
}

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
    base_config: Config,
    config_path: PathBuf,
    config_mtime: Option<SystemTime>,
    macros_path: PathBuf,
    macros_mtime: Option<SystemTime>,
    file_macros: HashMap<String, String>,
    last_reload_check: Option<Instant>,
    signals: Vec<IbusSignal>,
    holding_eaten_backspace: bool,
    preedit_visible: bool,
    pending_commit_suffix: Option<String>,
}

fn merge_config_macros(base: &Config, file_macros: &HashMap<String, String>) -> Config {
    let mut merged = base.clone();
    merged.macros.extend(file_macros.iter().map(|(k, v)| (k.clone(), v.clone())));
    merged
}

impl IbusEngineService {
    /// Khởi tạo dịch vụ IBus Engine với đường dẫn tệp cấu hình `config_path`.
    pub fn new(config_path: PathBuf) -> Self {
        let base_config = load_config_or_default(&config_path);
        let config_mtime = std::fs::metadata(&config_path).and_then(|m| m.modified()).ok();
        let macros_path = std::env::var_os("VNTYPE_MACROS")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| config_path.with_file_name("macros.txt"));
        let macros_mtime = std::fs::metadata(&macros_path).and_then(|m| m.modified()).ok();
        let file_macros = load_macros_file(&macros_path);
        let merged = merge_config_macros(&base_config, &file_macros);
        IbusEngineService {
            controller: AdapterController::new(merged, OutputMode::Preedit),
            base_config,
            config_path,
            config_mtime,
            macros_path,
            macros_mtime,
            file_macros,
            last_reload_check: None,
            signals: Vec::new(),
            holding_eaten_backspace: false,
            preedit_visible: false,
            pending_commit_suffix: None,
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
        // Trên Linux/IBus việc chuyển bộ gõ do desktop/IBus đảm nhận;
        // không chặn Alt+, hoặc Alt+Z của ứng dụng/hệ thống.
        if ev.alt && !ev.ctrl && !ev.meta && matches!(ev.key, Key::Char(',' | 'z' | 'Z')) {
            self.holding_eaten_backspace = false;
            return false;
        }
        self.reload_config_if_needed(false);
        if keyval == 0xff08 {
            self.pending_commit_suffix = None;
        }
        let effects = self.controller.process_key(ev.clone(), keyval);
        let handled = self.apply_effects(effects, keyval, keycode, state);
        if keyval == 0xff08 {
            if handled {
                self.holding_eaten_backspace = true;
            }
        } else {
            self.holding_eaten_backspace = false;
        }
        debug_log!(
            "KEY keyval={keyval:#x} keycode={keycode} state={state:#x} ev={ev:?} -> handled={handled} display={:?} adopted={} signals={:?}",
            self.controller.display_text(),
            self.controller.session().is_adopted(),
            self.signals,
        );
        handled
    }

    /// Xử lý tín hiệu `SetSurroundingText(text, cursor_chars, anchor_chars)` từ IBus.
    pub fn set_surrounding_text(&mut self, text: &str, cursor_chars: u32, anchor_chars: u32) {
        if !self.controller.display_text().is_empty() {
            return;
        }
        if cursor_chars != anchor_chars {
            self.pending_commit_suffix = None;
            debug_log!("SET_SURROUNDING selection cursor={cursor_chars} anchor={anchor_chars}");
            self.controller.set_surrounding_utf16(Some(""), 0, 1);
            return;
        }
        let Some((clean, c_chars)) = window_and_strip_surrounding(text, cursor_chars) else {
            self.pending_commit_suffix = None;
            self.controller.set_surrounding_utf16(None, 0, 0);
            return;
        };
        if let Some(suffix) = self.pending_commit_suffix.take() {
            let before_end =
                clean.char_indices().nth(c_chars as usize).map(|(i, _)| i).unwrap_or(clean.len());
            let before = &clean[..before_end];
            if !before.ends_with(&suffix) {
                let mut chars = suffix.chars();
                let last = chars.next_back();
                let without_last = chars.as_str();
                if last.is_some_and(|c| !c.is_alphanumeric())
                    && !without_last.is_empty()
                    && before.ends_with(without_last)
                {
                    self.pending_commit_suffix = Some(suffix);
                    debug_log!(
                        "SET_SURROUNDING ignored stale pre-commit snapshot window={clean:?} cursor={c_chars}"
                    );
                    return;
                }
            }
        }
        debug_log!("SET_SURROUNDING raw_bytes={} window={clean:?} cursor={c_chars}", text.len());
        let Some(c_u16) = char_to_utf16_offset(&clean, c_chars) else {
            self.controller.set_surrounding_utf16(None, 0, 0);
            return;
        };
        self.controller.set_surrounding_utf16(Some(&clean), c_u16, c_u16);
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
        debug_log!("SET_CONTENT_TYPE purpose={purpose} -> {kind:?}");
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
        if name == "vntype.app.settings" {
            let exe = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|dir| dir.join("vntype-linux-settings")))
                .filter(|p| p.is_file())
                .unwrap_or_else(|| PathBuf::from("vntype-linux-settings"));
            let _ = std::process::Command::new(exe)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            return;
        }

        if name == "vntype.macro.edit" {
            if let Err(_err) = ensure_macros_file(&self.macros_path) {
                debug_log!("MACRO_EDIT ensure_macros_file err={_err}");
                return;
            }
            self.macros_mtime = std::fs::metadata(&self.macros_path).and_then(|m| m.modified()).ok();
            let _ = std::process::Command::new("xdg-open")
                .arg(&self.macros_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            return;
        }

        let mut cfg = self.base_config.clone();
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
        self.base_config = cfg;
        let merged = merge_config_macros(&self.base_config, &self.file_macros);
        let effects = self.controller.apply_config(merged.clone());
        self.apply_effects(effects, 0, 0, 0);
        self.config_mtime = std::fs::metadata(&self.config_path).and_then(|m| m.modified()).ok();
        self.signals
            .push(IbusSignal::UpdateMenu { changed_key: name.to_string(), config: Box::new(merged) });
    }

    /// Xử lý sự kiện nhận tiêu điểm `FocusIn(app_id)`.
    pub fn focus_in(&mut self, app_id: Option<&str>) {
        debug_log!("FOCUS_IN app_id={app_id:?}");
        self.holding_eaten_backspace = false;
        self.pending_commit_suffix = None;
        self.reload_config_if_needed(false);
        let effects = self.controller.focus_changed(app_id);
        self.apply_effects(effects, 0, 0, 0);
    }

    /// Xử lý sự kiện mất tiêu điểm `FocusOut()`.
    pub fn focus_out(&mut self) {
        debug_log!("FOCUS_OUT");
        self.holding_eaten_backspace = false;
        self.pending_commit_suffix = None;
        let effects = self.controller.flush();
        self.apply_effects(effects, 0, 0, 0);
    }

    /// Xử lý sự kiện `Reset()` từ IBus.
    ///
    /// Lưu ý: IBus gửi `Reset` ngay sau khi phím Backspace xoá dấu cách. Nếu phiên vừa khôi phục
    /// từ đã chốt (`is_adopted()`), ta chỉ ẩn vùng gạch chân mà không xoá trạng thái từ đó.
    pub fn reset(&mut self) {
        debug_log!("RESET adopted={}", self.controller.session().is_adopted());
        self.pending_commit_suffix = None;
        if !self.controller.session().is_adopted() {
            self.controller.reset();
        }
        if self.preedit_visible {
            self.preedit_visible = false;
            self.signals.push(IbusSignal::HidePreeditText);
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>, keyval: u32, keycode: u32, state: u32) -> bool {
        let mut forward_original = false;
        let mut emitted_commit = false;
        for eff in effects {
            match eff {
                Effect::SetPreedit { text, cursor_utf16, .. } => {
                    self.preedit_visible = true;
                    self.pending_commit_suffix = None;
                    self.signals.push(IbusSignal::UpdatePreeditText { text, cursor_utf16, visible: true });
                }
                Effect::ClearPreedit => {
                    if self.preedit_visible {
                        self.preedit_visible = false;
                        self.signals.push(IbusSignal::HidePreeditText);
                    }
                }
                Effect::CommitText { text } => {
                    emitted_commit = true;
                    self.pending_commit_suffix = Self::track_commit_suffix(&text);
                    self.signals.push(IbusSignal::CommitText { text });
                }
                Effect::ReplaceText { delete_chars, insert, .. } => {
                    emitted_commit = true;
                    if delete_chars > 0 {
                        self.signals.push(IbusSignal::DeleteSurroundingText {
                            offset_chars: -(delete_chars as i32),
                            nchars: delete_chars,
                        });
                    }
                    if !insert.is_empty() {
                        self.pending_commit_suffix = Self::track_commit_suffix(&insert);
                        self.signals.push(IbusSignal::CommitText { text: insert });
                    } else {
                        self.pending_commit_suffix = None;
                    }
                }
                Effect::WaitMs { .. } => {}
                Effect::ForwardKey { raw_keycode } => {
                    let _ = raw_keycode;
                    self.pending_commit_suffix = None;
                    forward_original = true;
                }
            }
        }
        let has_system_modifier = (state & IBUS_SYSTEM_MODIFIER_MASK) != 0;
        let should_forward_sync = !has_system_modifier
            && keycode != 0
            && keyval != 0
            && forward_original
            && (emitted_commit || (keyval == 0xff08 && self.holding_eaten_backspace));
        if should_forward_sync {
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

    fn track_commit_suffix(text: &str) -> Option<String> {
        let mut chars = text.chars();
        let last = chars.next_back()?;
        if !last.is_alphanumeric() && !chars.as_str().is_empty() { Some(text.to_string()) } else { None }
    }

    fn reload_config_if_needed(&mut self, force: bool) {
        if !self.controller.display_text().is_empty() {
            return;
        }
        let now = Instant::now();
        if !force
            && let Some(prev) = self.last_reload_check
            && now.duration_since(prev) < Duration::from_secs(2)
        {
            return;
        }
        self.last_reload_check = Some(now);
        let cfg_mtime = std::fs::metadata(&self.config_path).and_then(|m| m.modified()).ok();
        let mac_mtime = std::fs::metadata(&self.macros_path).and_then(|m| m.modified()).ok();
        let mut dirty = false;

        if cfg_mtime != self.config_mtime {
            self.config_mtime = cfg_mtime;
            self.base_config = load_config_or_default(&self.config_path);
            dirty = true;
        }
        if mac_mtime != self.macros_mtime {
            self.macros_mtime = mac_mtime;
            self.file_macros = load_macros_file(&self.macros_path);
            dirty = true;
        }

        if dirty {
            let merged = merge_config_macros(&self.base_config, &self.file_macros);
            let effects = self.controller.apply_config(merged);
            self.apply_effects(effects, 0, 0, 0);
        }
    }
}

const MAX_SURROUNDING_BEFORE_CHARS: usize = 128;

fn window_and_strip_surrounding(text: &str, cursor_chars: u32) -> Option<(String, u32)> {
    let cursor = cursor_chars as usize;
    let start = cursor.saturating_sub(MAX_SURROUNDING_BEFORE_CHARS);
    let mut clean = String::with_capacity((cursor - start + 1).min(MAX_SURROUNDING_BEFORE_CHARS + 1) * 2);
    let mut clean_cursor = 0u32;
    let mut seen = 0usize;

    for (idx, ch) in text.chars().enumerate() {
        seen = idx + 1;
        if idx < start {
            continue;
        }
        if idx < cursor {
            if ch != '\u{200b}' {
                clean.push(ch);
                clean_cursor += 1;
            }
        } else if ch != '\u{200b}' {
            clean.push(ch);
            break;
        }
    }

    if seen < cursor {
        return None;
    }
    Some((clean, clean_cursor))
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

        // Reset khi đang adopted không xoá trạng thái từ đang sửa và không phát HidePreeditText thừa khi chưa có preedit
        svc.reset();
        assert!(svc.drain_signals().is_empty());
        assert!(svc.controller.session().is_adopted());

        // Khi Backspace xoá từ đã adopt, chỉ phát DeleteSurroundingText mà không phát HidePreeditText thừa
        assert!(svc.process_key_event(0xff08, 14, 0));
        assert_eq!(
            svc.drain_signals(),
            vec![IbusSignal::DeleteSurroundingText { offset_chars: -1, nchars: 1 }]
        );

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

        // Alt+, và Alt+Z không còn bật/tắt chế độ trong vntype-ibus mà đi thẳng qua ứng dụng
        assert!(!svc.process_key_event(b',' as u32, 0, crate::keymap::IBUS_MOD1_MASK));
        assert!(!svc.process_key_event(b'z' as u32, 0, crate::keymap::IBUS_MOD1_MASK));
        assert!(svc.drain_signals().is_empty());

        // Tổ hợp hệ thống (Super/Mod4+a, Ctrl+c) phải trả về false để OS/app xử lý trực tiếp
        assert!(!svc.process_key_event(b'a' as u32, 30, crate::keymap::IBUS_MOD4_MASK));
        assert!(svc.drain_signals().is_empty());

        assert!(svc.process_key_event(b't' as u32, 20, 0));
        let _ = svc.drain_signals();
        assert!(!svc.process_key_event(b'c' as u32, 46, crate::keymap::IBUS_CONTROL_MASK));
        assert_eq!(
            svc.drain_signals(),
            vec![IbusSignal::CommitText { text: "t".into() }, IbusSignal::HidePreeditText]
        );

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

    #[test]
    fn commit_before_forwarded_enter_preserves_fifo_order() {
        let mut svc = IbusEngineService::new(PathBuf::from("/nonexistent/config.json"));
        for b in b"lawngj" {
            assert!(svc.process_key_event(*b as u32, 0, 0));
        }
        let _ = svc.drain_signals();

        // Nhấn Enter (0xff0d, keycode 36) khi đang có preedit "lặng":
        // phải CommitText + HidePreeditText trước rồi mới ForwardKeyEvent và trả về true.
        assert!(svc.process_key_event(0xff0d, 36, 0));
        let sigs = svc.drain_signals();
        assert_eq!(
            sigs,
            vec![
                IbusSignal::CommitText { text: "lặng".into() },
                IbusSignal::HidePreeditText,
                IbusSignal::ForwardKeyEvent { keyval: 0xff0d, keycode: 36, state: 0 },
                IbusSignal::ForwardKeyEvent { keyval: 0xff0d, keycode: 36, state: IBUS_RELEASE_MASK },
            ]
        );
    }

    #[test]
    fn macros_txt_loads_reloads_and_does_not_pollute_config_json() {
        let dir = std::env::temp_dir().join(format!("vntype_ibus_macros_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("config.json");
        let macros_path = dir.join("macros.txt");

        std::fs::write(&macros_path, "ko=không\n").unwrap();
        let mut svc = IbusEngineService::new(cfg_path.clone());
        assert_eq!(svc.current_config().macros.get("ko").map(String::as_str), Some("không"));

        // Bật/tắt một tuỳ chọn trên menu không được ghi đè macros từ macros.txt vào config.json
        svc.property_activate("vntype.opt.w_as_u", 1);
        let saved_cfg = load_config_or_default(&cfg_path);
        assert!(saved_cfg.macros.is_empty());

        // Mô phỏng cập nhật macros.txt (lùi mtime trước rồi ghi mới để tránh trùng timestamp trên FS nhanh)
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&macros_path, "dc=được\n").unwrap();
        svc.macros_mtime = None;
        svc.reload_config_if_needed(true);
        assert_eq!(svc.current_config().macros.get("dc").map(String::as_str), Some("được"));
        assert!(!svc.current_config().macros.contains_key("ko"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_surrounding_ignores_zwsp_and_active_preedit_echo() {
        let mut svc = IbusEngineService::new(PathBuf::from("/nonexistent/config.json"));
        svc.set_surrounding_text("gõ \u{200b}\n", 4, 4);
        // Không bị lỗi offset khi loại bỏ '\u{200b}'
        svc.set_surrounding_text("tiêng\u{200b}", 6, 6);
        assert!(svc.process_key_event(b's' as u32, 0, 0));
        let sigs = svc.drain_signals();
        assert!(sigs.iter().any(|s| matches!(s, IbusSignal::CommitText { text } if text == "ếng")));

        // Khi đang có preedit dở dang, SetSurroundingText trễ từ app không được làm hỏng trạng thái
        svc.process_key_event(b' ' as u32, 0, 0);
        let _ = svc.drain_signals();
        assert!(svc.process_key_event(b't' as u32, 0, 0));
        svc.set_surrounding_text("rác ", 4, 4);
        assert!(svc.process_key_event(b'i' as u32, 0, 0));
        let sigs2 = svc.drain_signals();
        assert!(
            sigs2.iter().any(|s| matches!(s, IbusSignal::UpdatePreeditText { text, .. } if text == "ti"))
        );

        // Văn bản surrounding rất dài (10.000+ ký tự) được cắt cửa sổ gọn quanh con trỏ vẫn adopt đúng
        svc.process_key_event(b' ' as u32, 0, 0);
        let _ = svc.drain_signals();
        let long_text = format!("{} tiêng{}", "a ".repeat(5000), " xyz".repeat(100));
        let cursor = ("a ".repeat(5000) + " tiêng").chars().count() as u32;
        svc.set_surrounding_text(&long_text, cursor, cursor);
        assert!(svc.process_key_event(b's' as u32, 0, 0));
        let sigs3 = svc.drain_signals();
        assert!(sigs3.iter().any(|s| matches!(s, IbusSignal::CommitText { text } if text == "ếng")));
    }

    #[test]
    fn set_surrounding_ignores_stale_echo_missing_committed_space() {
        let mut svc = IbusEngineService::new(PathBuf::from("/nonexistent/config.json"));
        for b in b"Taij " {
            assert!(svc.process_key_event(*b as u32, 0, 0));
        }
        let sigs1 = svc.drain_signals();
        assert!(sigs1.iter().any(|s| matches!(s, IbusSignal::CommitText { text } if text == "Tại ")));

        // Client phát SetSurroundingText trễ (thiếu dấu cách vừa commit và còn kèm ZWSP)
        svc.set_surrounding_text("Tại\u{200b}", 3, 3);

        for b in b"sao " {
            assert!(svc.process_key_event(*b as u32, 0, 0));
        }
        let sigs2 = svc.drain_signals();
        assert!(!svc.controller.session().is_adopted());
        assert!(!sigs2.iter().any(|s| matches!(s, IbusSignal::DeleteSurroundingText { .. })));
        assert!(sigs2.iter().any(|s| matches!(s, IbusSignal::CommitText { text } if text == "sao ")));
    }
}
