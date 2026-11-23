//! Máy trạng thái quản lý phiên gõ tiếng Việt (`ImeSession`).
//!
//! Lưu ý tối ưu hiệu năng: Mọi thao tác cắt hậu tố văn bản xung quanh đều sử dụng
//! bộ lặp ngược trên chỉ số byte UTF-8 (`char_indices().rev()`) trong thời gian $O(k)$,
//! tuyệt đối không cấp phát `Vec<char>` cho toàn bộ đoạn văn.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;
use vntype_core::{Composer, InputMethodSpec, MAX_RAW, Status, TELEX, TransformOptions, diff, rebuild};
use vntype_syllable::upper;

use crate::macro_table::{expand_single_macro, match_phrase_macro};
use crate::ports::{AppProfile, Config, OutputMode, Ports, UnicodeForm, normalize_app_id, resolve_profile};

/// Sự kiện phím đã chuẩn hoá trong bộ gõ.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// Một ký tự văn bản Unicode.
    Char(char),
    /// Phím xoá lùi.
    Backspace,
    /// Phím xuống dòng.
    Enter,
    /// Phím thoát (khôi phục từ đang gõ về phím thô).
    Escape,
    /// Phím Tab.
    Tab,
    /// Phím mũi tên trái.
    Left,
    /// Phím mũi tên phải.
    Right,
    /// Phím mũi tên lên.
    Up,
    /// Phím mũi tên xuống.
    Down,
    /// Phím Home.
    Home,
    /// Phím End.
    End,
    /// Phím Delete.
    Delete,
    /// Phím PageUp.
    PageUp,
    /// Phím PageDown.
    PageDown,
    /// Các phím bổ trợ nhấn riêng lẻ (Shift, Control, Alt, Meta, Super, CapsLock...).
    Modifier,
    /// Các phím chức năng khác đóng vai trò làm ranh giới kết thúc từ.
    Other,
}

impl Key {
    /// Phân tích tên phím từ chuỗi ký tự chuẩn (ví dụ: `"a"`, `"Backspace"`, `"Enter"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use vntype_engine::Key;
    ///
    /// assert_eq!(Key::parse("a"), Key::Char('a'));
    /// assert_eq!(Key::parse("Backspace"), Key::Backspace);
    /// assert!(Key::parse("Left").is_nav());
    /// ```
    pub fn parse(name: &str) -> Key {
        let mut it = name.chars();
        if let (Some(c), None) = (it.next(), it.next()) {
            return Key::Char(c);
        }
        match name {
            "Backspace" => Key::Backspace,
            "Enter" => Key::Enter,
            "Escape" => Key::Escape,
            "Tab" => Key::Tab,
            "Left" => Key::Left,
            "Right" => Key::Right,
            "Up" => Key::Up,
            "Down" => Key::Down,
            "Home" => Key::Home,
            "End" => Key::End,
            "Delete" => Key::Delete,
            "PageUp" => Key::PageUp,
            "PageDown" => Key::PageDown,
            "Shift" | "Control" | "Alt" | "Meta" | "Super" | "CapsLock" | "Option" | "Command" | "Fn" => {
                Key::Modifier
            }
            _ => Key::Other,
        }
    }

    /// Kiểm tra xem phím có phải là phím điều hướng con trỏ hay không.
    pub fn is_nav(&self) -> bool {
        matches!(
            self,
            Key::Tab
                | Key::Left
                | Key::Right
                | Key::Up
                | Key::Down
                | Key::Home
                | Key::End
                | Key::Delete
                | Key::PageUp
                | Key::PageDown
        )
    }
}

/// Gói sự kiện bàn phím kèm trạng thái các phím bổ trợ.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// Phím chính được nhấn.
    pub key: Key,
    /// Trạng thái phím Control đang giữ.
    pub ctrl: bool,
    /// Trạng thái phím Alt / Option đang giữ.
    pub alt: bool,
    /// Trạng thái phím Super / Command / Windows đang giữ.
    pub meta: bool,
}

impl KeyEvent {
    /// Tạo sự kiện phím không có phím bổ trợ.
    pub fn new(key: Key) -> Self {
        KeyEvent { key, ctrl: false, alt: false, meta: false }
    }

    /// Tạo sự kiện phím cho một ký tự đơn.
    pub fn ch(c: char) -> Self {
        KeyEvent::new(Key::Char(c))
    }

    /// Tạo sự kiện phím từ tên chuỗi.
    pub fn parse(name: &str) -> Self {
        KeyEvent::new(Key::parse(name))
    }
}

/// Phân loại ngữ cảnh trường nhập liệu hiện tại.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FieldKind {
    /// Ô văn bản thông thường.
    #[default]
    Text,
    /// Ô mật khẩu hoặc mã PIN nhạy cảm (luôn tắt bộ gõ và không giữ vùng soạn).
    Password,
    /// Ô chỉ nhập số hoặc số điện thoại.
    Number,
    /// Ô nhập địa chỉ thư điện tử.
    Email,
    /// Thanh địa chỉ trình duyệt / Omnibox (cho phép gõ tiếng Việt tìm kiếm nhưng bật chế độ URL-safe).
    Url,
    /// Giao diện dòng lệnh Terminal (ưu tiên chế độ Preedit nguyên tử).
    Terminal,
}

impl FieldKind {
    /// Kiểm tra trường nhập liệu có bị vô hiệu hoá hoàn toàn bộ gõ tiếng Việt hay không.
    pub fn is_disabled(self) -> bool {
        matches!(self, FieldKind::Password | FieldKind::Number | FieldKind::Email)
    }
}

/// Hành động đầu ra của [`ImeSession`] sau khi xử lý một sự kiện.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Bộ gõ không can thiệp, chuyển phím gốc cho hệ điều hành.
    PassThrough,
    /// Cập nhật vùng văn bản gạch chân tạm thời (Preedit).
    Preedit {
        /// Chuỗi hiển thị trong vùng gạch chân.
        text: String,
        /// Vị trí con trỏ tính theo số lượng `char` từ đầu vùng gạch chân.
        cursor: usize,
    },
    /// Xoá lùi một số ký tự trước con trỏ và chèn chuỗi văn bản mới.
    Replace {
        /// Số lượng ký tự (`char`) cần xoá lùi.
        delete_before: usize,
        /// Chuỗi ký tự cần chèn vào.
        insert: String,
        /// Nếu `true`, sau khi thay thế xong sẽ cho phép phím gốc đi tiếp vào ứng dụng.
        pass_key: bool,
    },
    /// Chốt chuỗi văn bản chính thức vào ứng dụng và xoá vùng gạch chân.
    Commit {
        /// Chuỗi văn bản cần chốt.
        text: String,
        /// Nếu `true`, sau khi chốt xong sẽ cho phép phím gốc đi tiếp vào ứng dụng.
        pass_key: bool,
    },
}

#[derive(Debug, Clone)]
struct LastWord {
    raw: Vec<char>,
    word: String,
    literal: bool,
    sep: char,
}

fn is_sentence_end(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…')
}

fn is_sentence_closer(c: char) -> bool {
    matches!(c, '"' | '\'' | ')' | ']' | '»' | '”' | '’' | '›')
}

/// Cắt tối đa `max_chars` ký tự cuối cùng của chuỗi `s` trong thời gian $O(\text{max\_chars})$ mà không cấp phát `Vec<char>`.
fn tail_chars_slice(s: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    match s.char_indices().rev().nth(max_chars - 1) {
        Some((byte_idx, _)) => &s[byte_idx..],
        None => s,
    }
}

/// Máy trạng thái phiên gõ tiếng Việt cho một ngữ cảnh nhập liệu.
pub struct ImeSession {
    ports: Ports,
    mode: OutputMode,
    config: Config,
    profile: AppProfile,
    raw: Vec<char>,
    word: String,
    literal: bool,
    escaped: bool,
    cap_next: bool,
    pending_end: bool,
    enabled: bool,
    field: FieldKind,
    last: Option<LastWord>,
    last_macro: Option<(String, String)>,
    history: String,
    off_raw: Vec<char>,
    composer: Option<Composer>,
    adopted: Option<(String, usize)>,
    adopt_allowed: bool,
}

impl ImeSession {
    /// Khởi tạo một phiên gõ mới với các cổng `ports` và chế độ xuất chữ `mode`.
    pub fn new(ports: Ports, mode: OutputMode) -> Self {
        let config = ports.config.load();
        let mut s = ImeSession {
            ports,
            mode,
            config,
            profile: AppProfile::default(),
            raw: Vec::new(),
            word: String::new(),
            literal: false,
            escaped: false,
            cap_next: false,
            pending_end: false,
            enabled: true,
            field: FieldKind::Text,
            last: None,
            last_macro: None,
            history: String::new(),
            off_raw: Vec::new(),
            composer: None,
            adopted: None,
            adopt_allowed: true,
        };
        let app = s.ports.app.current_app();
        s.focus_changed(app.as_deref());
        s
    }

    /// Trả về chế độ xuất chữ hiện tại (`Preedit` hoặc `Replace`).
    pub fn mode(&self) -> OutputMode {
        self.mode
    }

    /// Trả về tham chiếu tới cấu hình đang áp dụng.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Trả về tham chiếu tới hồ sơ của ứng dụng hiện hành.
    pub fn profile(&self) -> &AppProfile {
        &self.profile
    }

    /// Kiểm tra xem chế độ gõ tiếng Việt có đang bật hay không.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Kiểm tra từ hiện tại có đang ở trạng thái gõ thô (literal) hay không.
    pub fn is_literal(&self) -> bool {
        self.literal
    }

    /// Kiểm tra từ hiện tại có phải là từ được nạp lại từ văn bản đã chốt trước đó (`adopted`) không.
    pub fn is_adopted(&self) -> bool {
        self.adopted.is_some()
    }

    /// Trả về loại trường nhập liệu hiện tại.
    pub fn field_kind(&self) -> FieldKind {
        self.field
    }

    /// Trả về tham chiếu tới tập cổng ra [`Ports`].
    pub fn ports(&self) -> &Ports {
        &self.ports
    }

    /// Trả về chuỗi hiển thị của từ đang gõ theo đúng dạng Unicode đầu ra (`NFC` hoặc `NFD`).
    pub fn display(&self) -> String {
        self.out(&self.word)
    }

    /// Trả về chuỗi các phím thô của từ đang gõ.
    pub fn raw_string(&self) -> String {
        self.raw.iter().collect()
    }

    fn im(&self) -> &'static InputMethodSpec {
        let name = self.profile.input_method.as_deref().unwrap_or(&self.config.input_method);
        InputMethodSpec::by_name(name).unwrap_or(&TELEX)
    }

    fn url_safe(&self) -> bool {
        self.field == FieldKind::Url
    }

    fn macro_on(&self) -> bool {
        self.config.macro_enabled && !self.url_safe() && !self.literal
    }

    fn starts_url_segment(&self) -> bool {
        matches!(self.history.chars().last(), Some('.' | '@' | '/' | ':' | '\\' | '-' | '_'))
    }

    fn opt(&self) -> TransformOptions {
        let c = &self.config;
        let url_safe = self.url_safe();
        TransformOptions {
            tone_style: self.profile.tone_style.unwrap_or(c.tone_style),
            spell_check: self.profile.spell_check.unwrap_or(c.spell_check),
            w_as_u: c.w_as_u && !url_safe,
            bracket_keys: c.bracket_keys && !url_safe,
            quick_telex: c.quick_telex && !url_safe,
            allow_coda_k: c.allow_coda_k,
        }
    }

    fn out(&self, s: &str) -> String {
        let charset = self.profile.charset.as_deref().unwrap_or(&self.config.charset);
        let key = vntype_core::normalize_charset(charset);
        if key == "unicode" {
            match self.config.unicode_form {
                UnicodeForm::Nfc => s.nfc().collect(),
                UnicodeForm::Nfd => s.nfd().collect(),
            }
        } else {
            vntype_core::encode_charset(s, key)
        }
    }

    fn compute(&mut self, is_final: bool) -> String {
        if self.literal || self.smart_caps_literal() {
            return self.raw.iter().collect();
        }
        let stale = match &self.composer {
            Some(c) => c.raw() != self.raw.as_slice(),
            None => true,
        };
        if stale {
            self.composer = Some(Composer::with_raw(self.im(), self.opt(), &self.raw));
        }
        let res = match &self.composer {
            Some(c) => c.result(is_final),
            None => unreachable!(),
        };
        if !res.rendered.is_empty() {
            self.escaped = true;
        }
        if res.restored {
            self.literal = true;
            if let Some((word, n)) = &self.adopted {
                let mut s = word.clone();
                s.extend(self.raw.get(*n..).unwrap_or(&[]).iter());
                return s;
            }
        }
        res.text
    }

    fn smart_caps_literal(&self) -> bool {
        let r = &self.raw;
        if !(self.config.smart_caps && r.len() >= 2 && r[0].is_uppercase() && r[1].is_uppercase()) {
            return false;
        }
        !matches!(
            (r[0], r[1]),
            ('D', 'D') | ('A', 'A') | ('A', 'W') | ('E', 'E') | ('O', 'O') | ('O', 'W') | ('U', 'W')
        )
    }

    /// Xoá sạch trạng thái của từ đang gõ mà không phát sinh [`Action`].
    pub fn reset(&mut self) {
        self.raw.clear();
        self.word.clear();
        self.literal = false;
        self.escaped = false;
        self.composer = None;
        self.adopted = None;
    }

    /// Chốt vùng văn bản đang soạn dở và trả về [`Action`] tương ứng.
    pub fn flush_composition(&mut self) -> Action {
        self.flush()
    }

    /// Thay đổi chế độ xuất chữ (`Preedit` hoặc `Replace`) sau khi đã chốt từ đang gõ dở.
    pub fn set_output_mode(&mut self, mode: OutputMode) -> Action {
        let act = self.flush();
        self.mode = mode;
        self.composer = None;
        act
    }

    fn is_word_char(&self, ch: char) -> bool {
        if vntype_syllable::lower(ch).is_ascii_lowercase() {
            return true;
        }
        if !self.raw.is_empty() && self.im().extra_word_key(ch) {
            return true;
        }
        if self.config.bracket_keys && self.im().insert_for(ch).is_some() {
            return true;
        }
        if self.config.macro_enabled
            && self.config.macro_digits
            && !self.im().digit_keys
            && ch.is_ascii_digit()
            && !self.raw.is_empty()
        {
            return true;
        }
        self.im().digit_keys && ch.is_ascii_digit() && !self.raw.is_empty()
    }

    /// Nạp lại cấu hình từ cổng [`crate::ports::ConfigStore`] sau khi đã chốt từ đang gõ.
    pub fn reload_config(&mut self) -> Action {
        let act = self.flush();
        self.config = self.ports.config.load();
        self.composer = None;
        self.last_macro = None;
        act
    }

    /// Cập nhật ngữ cảnh khi người dùng chuyển sang ứng dụng khác (`app_id`).
    pub fn focus_changed(&mut self, app_id: Option<&str>) -> Action {
        let act = self.flush();
        self.cap_next = false;
        self.pending_end = false;
        self.profile = resolve_profile(&self.config, app_id);
        let key = normalize_app_id(app_id);
        let c = &self.config;
        let excluded = |a: &String| normalize_app_id(Some(a)) == key;
        self.enabled = if !key.is_empty() && c.excluded_apps.iter().any(excluded) {
            false
        } else if let (Some(v), false) = (self.profile.vietnamese, c.app_modes.contains_key(&key)) {
            v
        } else {
            c.app_modes.get(&key).copied().unwrap_or(c.default_vietnamese)
        };
        act
    }

    /// Cập nhật phân loại trường nhập liệu (`Text`, `Password`, `Url`, ...).
    pub fn set_field_kind(&mut self, kind: FieldKind) -> Action {
        let act = self.flush();
        self.field = kind;
        act
    }

    /// Thông báo con trỏ văn bản đã bị dịch chuyển do nhấp chuột hoặc thao tác bên ngoài.
    pub fn cursor_moved(&mut self) -> Action {
        self.cap_next = false;
        self.pending_end = false;
        self.adopt_allowed = false;
        self.flush()
    }

    /// Đảo trạng thái bật/tắt gõ tiếng Việt và lưu lại theo ứng dụng hiện hành.
    pub fn toggle_vietnamese(&mut self) -> Action {
        let act = self.flush();
        self.enabled = !self.enabled;
        let app = normalize_app_id(self.ports.app.current_app().as_deref());
        if !app.is_empty() {
            self.config.app_modes.insert(app, self.enabled);
            self.ports.config.save(&self.config);
        }
        act
    }

    fn flush(&mut self) -> Action {
        let text = self.display();
        let adopted = self.adopted.is_some();
        self.reset();
        self.last = None;
        self.last_macro = None;
        self.history.clear();
        self.off_raw.clear();
        if self.mode == OutputMode::Preedit && !text.is_empty() && !adopted {
            return Action::Commit { text, pass_key: false };
        }
        Action::PassThrough
    }

    /// Xử lý một sự kiện bàn phím và trả về hành động [`Action`].
    pub fn process_key(&mut self, ev: &KeyEvent) -> Action {
        if self.field.is_disabled() {
            return Action::PassThrough;
        }
        if !self.enabled {
            return self.off_mode_key(ev);
        }
        if ev.key == Key::Modifier {
            return Action::PassThrough;
        }
        if ev.key != Key::Backspace {
            self.last_macro = None;
        }
        if ev.ctrl || ev.alt || ev.meta || ev.key.is_nav() {
            self.cap_next = false;
            self.pending_end = false;
            self.last = None;
            return self.boundary_passthrough();
        }
        match ev.key {
            Key::Backspace => self.backspace(),
            Key::Escape => self.escape(),
            Key::Enter => {
                let act = self.word_end(None);
                self.cap_next = self.config.auto_capitalize;
                self.pending_end = false;
                act
            }
            Key::Char(c) if self.is_word_char(c) => self.letter(c),
            Key::Char(c) => self.separator(c),
            _ => self.boundary_passthrough(),
        }
    }

    fn off_mode_key(&mut self, ev: &KeyEvent) -> Action {
        if ev.key == Key::Modifier || ev.ctrl || ev.alt || ev.meta || ev.key.is_nav() {
            self.off_raw.clear();
            self.last_macro = None;
            return Action::PassThrough;
        }
        let macro_on = self.macro_on() && self.config.macro_in_off_mode;
        if macro_on
            && let Key::Char(ch) = ev.key
            && (ch.is_alphabetic() || ch.is_ascii_digit())
        {
            if self.off_raw.len() < MAX_RAW {
                self.off_raw.push(ch);
            }
            return Action::PassThrough;
        }
        if macro_on && !self.off_raw.is_empty() {
            let raw: String = self.off_raw.iter().collect();
            let mac = expand_single_macro(&self.config.macros, &raw, &raw);
            self.off_raw.clear();
            if let Some(m) = mac {
                let sep = if let Key::Char(ch) = ev.key { Some(ch) } else { None };
                if self.mode == OutputMode::Replace {
                    self.history.push_str(&m);
                    if let Some(s) = sep {
                        self.history.push(s);
                    }
                    let key_len = raw.chars().count();
                    return Action::Replace {
                        delete_before: key_len,
                        insert: m,
                        pass_key: sep.is_some() || ev.key == Key::Enter,
                    };
                }
            }
        }
        self.off_raw.clear();
        Action::PassThrough
    }

    fn separator(&mut self, ch: char) -> Action {
        let act = self.word_end(Some(ch));
        if is_sentence_end(ch) {
            self.pending_end = true;
        } else if is_sentence_closer(ch) {
            // Giữ trạng thái kết câu qua dấu đóng ngoặc
        } else if ch.is_whitespace() {
            if self.pending_end || matches!(ch, '\n' | '\r') {
                self.cap_next = self.config.auto_capitalize;
            }
            self.pending_end = false;
        } else {
            self.pending_end = false;
            self.cap_next = false;
        }
        act
    }

    fn show(&mut self, new: String, typed: Option<char>) -> Action {
        let old_out = self.display();
        self.word = new;
        let new_out = self.display();
        if self.mode == OutputMode::Preedit {
            if self.adopted.is_some() {
                let e = diff(&old_out, &new_out);
                return Action::Replace {
                    delete_before: e.delete_before,
                    insert: e.insert,
                    pass_key: false,
                };
            }
            let cursor = new_out.chars().count();
            return Action::Preedit { text: new_out, cursor };
        }
        let e = diff(&old_out, &new_out);
        if let Some(t) = typed {
            let mut it = e.insert.chars();
            if e.delete_before == 0 && it.next() == Some(t) && it.next().is_none() {
                return Action::PassThrough;
            }
        }
        Action::Replace { delete_before: e.delete_before, insert: e.insert, pass_key: false }
    }

    fn show_after_backspace(&mut self, was_adopted: bool) -> Action {
        let text = self.compute(false);
        if was_adopted {
            self.adopted = Some((text.clone(), self.raw.len()));
        }
        self.show(text, None)
    }

    fn letter(&mut self, key: char) -> Action {
        let mut ch = key;
        if self.raw.is_empty() {
            if self.cap_next {
                ch = upper(ch);
            }
            self.cap_next = false;
            self.pending_end = false;
            self.adopt_previous_word();
            self.literal = self.raw.is_empty() && self.starts_url_segment();
        }
        self.raw.push(ch);
        let n = self.raw.len();
        if let Some(c) = self.composer.as_mut()
            && c.raw() == &self.raw[..n - 1]
        {
            c.push(ch);
        }
        if self.raw.len() > MAX_RAW {
            self.literal = true;
        }
        let text = self.compute(false);
        self.show(text, if ch == key { Some(ch) } else { None })
    }

    fn adopt_previous_word(&mut self) {
        let mut allowed = self.config.edit_previous_word;
        if let Some(p) = self.profile.edit_previous_word {
            allowed = allowed && p;
        }
        if !allowed || !self.adopt_allowed {
            return;
        }
        let before = match self.ports.surrounding.before_cursor(MAX_RAW + 1) {
            Some(b) if !b.is_empty() => b,
            _ => return,
        };
        let tail = tail_chars_slice(&before, MAX_RAW + 1);
        let mut word_start_byte = tail.len();
        for (byte_idx, ch) in tail.char_indices().rev() {
            if ch.is_alphabetic() || canonical_combining_class(ch) != 0 {
                word_start_byte = byte_idx;
            } else {
                break;
            }
        }
        let on_screen = &tail[word_start_byte..];
        if on_screen.is_empty() || self.out(on_screen) != on_screen {
            return;
        }
        let word: String = on_screen.nfc().collect();
        if word.chars().count() > MAX_RAW / 2 {
            return;
        }
        if let Some(raw) = rebuild(&word, self.im(), &self.opt())
            && !raw.is_empty()
        {
            let n = raw.len();
            self.raw = raw;
            self.word = word.clone();
            self.adopted = Some((word, n));
        }
    }

    fn backspace(&mut self) -> Action {
        if self.raw.is_empty() {
            self.history.clear();
            self.off_raw.clear();
            if self.config.macro_undo
                && self.mode == OutputMode::Replace
                && let Some((inserted, restored)) = self.last_macro.take()
            {
                let n = inserted.chars().count();
                if let Some(before) = self.ports.surrounding.before_cursor(n)
                    && self.out(&before) == inserted
                {
                    return Action::Replace { delete_before: n, insert: restored, pass_key: false };
                }
            }
            let restore_after_space =
                self.profile.restore_after_space.unwrap_or(self.config.restore_after_space);
            if let Some(last) = self.last.take()
                && restore_after_space
                && last.sep.len_utf8() > 0
            {
                self.raw = last.raw;
                self.word = last.word;
                self.literal = last.literal;
                self.adopted = if self.mode == OutputMode::Preedit {
                    Some((self.word.clone(), self.raw.len()))
                } else {
                    None
                };
            }
            return Action::PassThrough;
        }
        let mut t: Vec<char> = self.word.chars().collect();
        t.pop();
        let target: String = t.iter().collect();
        if target.is_empty() {
            let act = self.show(String::new(), None);
            self.reset();
            return act;
        }
        let was_adopted = self.adopted.is_some();
        if self.literal
            && self.raw.len() >= 2
            && self.raw[self.raw.len() - 1].eq_ignore_ascii_case(&self.raw[self.raw.len() - 2])
        {
            let candidate: Vec<char> = self.raw[..self.raw.len() - 1].to_vec();
            let probe = Composer::with_raw(self.im(), self.opt(), &candidate);
            let res = probe.result(false);
            self.adopted = None;
            self.composer = None;
            self.raw = candidate;
            self.literal = res.restored;
            self.escaped = false;
            return self.show_after_backspace(was_adopted);
        }
        let raw = if self.escaped || !self.literal {
            rebuild(&target, self.im(), &self.opt())
        } else if self.raw.iter().collect::<String>() == self.word
            && !self.ports.dictionary.is_english(&self.word.to_lowercase())
        {
            let candidate: Vec<char> = self.raw[..self.raw.len() - 1].to_vec();
            let probe = Composer::with_raw(self.im(), self.opt(), &candidate);
            let res = probe.result(false);
            if !res.restored && res.text != target && res.status != Status::Invalid {
                Some(candidate)
            } else {
                None
            }
        } else {
            None
        };
        self.adopted = None;
        match raw {
            Some(r) => {
                self.raw = r;
                self.literal = false;
                self.escaped = false;
            }
            None => {
                self.raw = t;
                self.literal = true;
            }
        }
        self.show_after_backspace(was_adopted)
    }

    fn escape(&mut self) -> Action {
        if self.raw.is_empty() {
            return Action::PassThrough;
        }
        let raw: String = self.raw.iter().collect();
        self.finish(raw, None, false)
    }

    fn word_end(&mut self, sep: Option<char>) -> Action {
        self.last_macro = None;
        if self.raw.is_empty() {
            self.last = None;
            if let Some(s) = sep {
                self.history.push(s);
                if self.history.len() > 512 {
                    self.history = tail_chars_slice(&self.history, 256).to_string();
                }
            }
            return Action::PassThrough;
        }
        let mut fin = self.compute(true);
        let raw_s: String = self.raw.iter().collect();
        if self.macro_on() && self.mode == OutputMode::Replace {
            let phrase = format!("{}{}", self.history, fin);
            if let Some((key, expansion, prefix)) =
                match_phrase_macro(&self.config.macros, &phrase, self.word.chars().count())
            {
                let sep_s = sep.map(|c| c.to_string()).unwrap_or_default();
                let inserted = format!("{expansion}{sep_s}");
                self.last_macro = sep.map(|_| (inserted.clone(), format!("{key}{sep_s}")));
                self.history = format!("{prefix}{inserted}");
                self.reset();
                return Action::Replace {
                    delete_before: key.chars().count(),
                    insert: expansion,
                    pass_key: sep.is_some(),
                };
            }
        }
        let mac = if self.macro_on() {
            expand_single_macro(&self.config.macros, &self.word, &self.word)
                .or_else(|| expand_single_macro(&self.config.macros, &raw_s, &raw_s))
        } else {
            None
        };
        if let Some(m) = mac {
            if let Some(s) = sep
                && self.config.macro_undo
            {
                let mut inserted = self.out(&m);
                inserted.push(s);
                let mut restored = raw_s.clone();
                restored.push(s);
                self.last_macro = Some((inserted, restored));
            }
            fin = m;
        }
        self.finish(fin, sep, true)
    }

    fn finish(&mut self, fin: String, sep: Option<char>, pass_key: bool) -> Action {
        let old_out = self.display();
        let final_out = self.out(&fin);
        self.last = match sep {
            Some(s) if fin == self.word => Some(LastWord {
                raw: self.raw.clone(),
                word: self.word.clone(),
                literal: self.literal,
                sep: s,
            }),
            _ => None,
        };
        if let Some(s) = sep {
            self.history.push_str(&final_out);
            self.history.push(s);
            if self.history.len() > 512 {
                self.history = tail_chars_slice(&self.history, 256).to_string();
            }
        }
        let adopted = self.adopted.is_some();
        self.reset();
        if self.mode == OutputMode::Preedit {
            if adopted {
                let mut out = final_out;
                if let Some(s) = sep {
                    out.push(s);
                }
                let e = diff(&old_out, &out);
                return Action::Replace {
                    delete_before: e.delete_before,
                    insert: e.insert,
                    pass_key: pass_key && sep.is_none(),
                };
            }
            return match sep {
                None => Action::Commit { text: final_out, pass_key },
                Some(s) => {
                    let mut text = final_out;
                    text.push(s);
                    Action::Commit { text, pass_key: false }
                }
            };
        }
        if final_out == old_out {
            return Action::PassThrough;
        }
        let e = diff(&old_out, &final_out);
        Action::Replace { delete_before: e.delete_before, insert: e.insert, pass_key }
    }

    fn boundary_passthrough(&mut self) -> Action {
        let text = self.display();
        let adopted = self.adopted.is_some();
        self.reset();
        if self.mode == OutputMode::Preedit && !text.is_empty() && !adopted {
            return Action::Commit { text, pass_key: true };
        }
        Action::PassThrough
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::shared_dictionary;
    use crate::ports::{MemoryConfigStore, SharedConfigStore, StaticAppContext, SurroundingText};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct MockSurrounding(Rc<RefCell<String>>);

    impl SurroundingText for MockSurrounding {
        fn before_cursor(&self, n: usize) -> Option<String> {
            let s = self.0.borrow();
            Some(tail_chars_slice(&s, n).to_string())
        }
    }

    fn make_session(cfg: Config, mode: OutputMode, app: Option<&str>) -> (ImeSession, Rc<RefCell<String>>) {
        let store = SharedConfigStore(Rc::new(MemoryConfigStore::new(cfg)));
        let surr_buf = Rc::new(RefCell::new(String::new()));
        let ports = Ports {
            config: Box::new(store),
            app: Box::new(StaticAppContext(app.map(str::to_string))),
            surrounding: Box::new(MockSurrounding(surr_buf.clone())),
            dictionary: Box::new(shared_dictionary()),
        };
        (ImeSession::new(ports, mode), surr_buf)
    }

    fn type_str(s: &mut ImeSession, text: &str) -> Vec<Action> {
        text.chars().map(|c| s.process_key(&KeyEvent::ch(c))).collect()
    }

    #[test]
    fn preedit_typing_commit_and_escape() {
        let (mut s, _) = make_session(Config::default(), OutputMode::Preedit, None);
        let acts = type_str(&mut s, "tieengs");
        assert_eq!(acts.last(), Some(&Action::Preedit { text: "tiếng".into(), cursor: 5 }));

        let commit = s.process_key(&KeyEvent::ch(' '));
        assert_eq!(commit, Action::Commit { text: "tiếng ".into(), pass_key: false });

        type_str(&mut s, "vieejt");
        let esc = s.process_key(&KeyEvent::new(Key::Escape));
        assert_eq!(esc, Action::Commit { text: "vieejt".into(), pass_key: false });
    }

    #[test]
    fn replace_mode_typing_and_backspace() {
        let (mut s, _) = make_session(Config::default(), OutputMode::Replace, None);
        let acts = type_str(&mut s, "tieengs");
        assert_eq!(
            acts.last(),
            Some(&Action::Replace { delete_before: 3, insert: "ếng".into(), pass_key: false })
        );
        let bs = s.process_key(&KeyEvent::new(Key::Backspace));
        assert_eq!(bs, Action::Replace { delete_before: 1, insert: "".into(), pass_key: false });
    }

    #[test]
    fn adopt_previous_word_and_edit_tone() {
        let (mut s, surr) = make_session(Config::default(), OutputMode::Replace, None);
        *surr.borrow_mut() = "xin tiêng".into();
        let act = s.process_key(&KeyEvent::ch('s'));
        assert!(s.is_adopted());
        assert_eq!(act, Action::Replace { delete_before: 3, insert: "ếng".into(), pass_key: false });
    }

    #[test]
    fn focus_excluded_and_toggle_persistence() {
        let mut cfg = Config::default();
        cfg.excluded_apps.push("kitty".into());
        let (mut s, _) = make_session(cfg, OutputMode::Preedit, Some("net.kovidgoyal.kitty"));
        assert!(!s.is_enabled());

        s.focus_changed(Some("chrome"));
        assert!(s.is_enabled());
        s.toggle_vietnamese();
        assert!(!s.is_enabled());
        // `toggle_vietnamese` lưu theo `ports.app.current_app()` (đang là `kitty`)
        assert_eq!(s.config().app_modes.get("kitty"), Some(&false));
    }

    #[test]
    fn field_kind_password_and_url() {
        let (mut s, _) = make_session(Config::default(), OutputMode::Preedit, None);
        s.set_field_kind(FieldKind::Password);
        assert_eq!(s.process_key(&KeyEvent::ch('a')), Action::PassThrough);

        let mut cfg = Config { w_as_u: true, ..Config::default() };
        cfg.macros.insert("ko".into(), "không".into());
        let (mut s_url, _) = make_session(cfg, OutputMode::Preedit, None);
        s_url.set_field_kind(FieldKind::Url);
        assert_eq!(s_url.process_key(&KeyEvent::ch('w')), Action::Preedit { text: "w".into(), cursor: 1 });
    }

    #[test]
    fn macro_expand_and_off_mode() {
        let mut cfg = Config { macro_in_off_mode: true, default_vietnamese: false, ..Config::default() };
        cfg.macros.insert("ko".into(), "không".into());
        let (mut s, _) = make_session(cfg, OutputMode::Replace, None);
        assert!(!s.is_enabled());
        type_str(&mut s, "ko");
        let act = s.process_key(&KeyEvent::ch(' '));
        assert_eq!(act, Action::Replace { delete_before: 2, insert: "không".into(), pass_key: true });
    }
}
