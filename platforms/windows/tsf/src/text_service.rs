//! Cài đặt dịch vụ `ITfTextInputProcessorEx`, `ITfThreadMgrEventSink` và `ITfKeyEventSink`
//! bằng Rust thuần, đóng gói trong một tệp `vntype_tsf.dll` duy nhất.

use vntype_adapter::{AdapterController, utf16_len};
use vntype_engine::{Config, Key, KeyEvent, OutputMode};

use crate::composition::CompositionTracker;
use crate::edit_session::{TsfContextBackend, apply_effects_in_edit_session};
use crate::input_scope::resolve_field_kind_from_scopes;

/// Bộ dịch vụ văn bản TSF (Text Service) gắn với một luồng giao diện Windows (`ITfThreadMgr`).
pub struct VntypeTextService {
    controller: AdapterController,
    composition: CompositionTracker,
    active_app: String,
}

impl VntypeTextService {
    /// Khởi tạo một thực thể `VntypeTextService` với cấu hình `config`.
    pub fn new(config: Config) -> Self {
        VntypeTextService {
            controller: AdapterController::new(config, OutputMode::Preedit),
            composition: CompositionTracker::default(),
            active_app: String::new(),
        }
    }

    /// Xử lý khi cửa sổ hoặc tài liệu TSF nhận tiêu điểm (`OnSetFocus`).
    pub fn on_set_focus<B: TsfContextBackend>(
        &mut self,
        app_name: &str,
        input_scopes: &[i32],
        backend: &mut B,
    ) {
        self.active_app = app_name.to_string();
        let focus_effects = self.controller.focus_changed(Some(app_name));
        apply_effects_in_edit_session(&focus_effects, &mut self.composition, backend);

        let kind = resolve_field_kind_from_scopes(input_scopes, app_name);
        let field_effects = self.controller.set_field_kind(kind);
        apply_effects_in_edit_session(&field_effects, &mut self.composition, backend);
    }

    /// Xử lý khi cửa sổ TSF mất tiêu điểm (`OnKillFocus`), chốt vùng soạn thảo nếu còn.
    pub fn on_kill_focus<B: TsfContextBackend>(&mut self, backend: &mut B) {
        let effects = self.controller.flush();
        apply_effects_in_edit_session(&effects, &mut self.composition, backend);
    }

    /// Xử lý sự kiện nhấn phím `ITfKeyEventSink::OnKeyDown`.
    ///
    /// Trả về `true` (`pfEaten = TRUE`) nếu TSF đã tiêu thụ phím, `false` nếu trả phím cho ứng dụng.
    pub fn on_key_down<B: TsfContextBackend>(
        &mut self,
        key_event: KeyEvent,
        vk_code: u32,
        surrounding_before: Option<&str>,
        backend: &mut B,
    ) -> bool {
        if !self.composition.active {
            if let Some(before) = surrounding_before {
                let u16_len = utf16_len(before);
                self.controller.set_surrounding_utf16(Some(before), u16_len, u16_len);
            } else {
                self.controller.set_surrounding_utf16(None, 0, 0);
            }
        }
        let effects = self.controller.process_key(key_event, vk_code);
        apply_effects_in_edit_session(&effects, &mut self.composition, backend)
    }

    /// Xử lý khi ứng dụng bên ngoài chủ động đóng vùng composition (`OnCompositionTerminated`).
    pub fn on_composition_terminated(&mut self) {
        if self.composition.ending_internally {
            return;
        }
        self.composition.clear();
        self.controller.reset();
    }

    /// Ánh xạ mã phím ảo Windows (`VK_*`) và ký tự đã dịch qua `ToUnicode` sang [`Key`].
    pub fn map_virtual_key(vk: u16, unicode_char: Option<char>) -> Key {
        match vk {
            0x08 => Key::Backspace,
            0x0D => Key::Enter,
            0x09 => Key::Tab,
            0x1B => Key::Escape,
            0x25 => Key::Left,
            0x26 => Key::Up,
            0x27 => Key::Right,
            0x28 => Key::Down,
            0x24 => Key::Home,
            0x23 => Key::End,
            0x2E => Key::Delete,
            0x21 => Key::PageUp,
            0x22 => Key::PageDown,
            0x10..=0x14 | 0x5B..=0x5C | 0xA0..=0xA5 => Key::Modifier,
            _ => match unicode_char {
                Some(c) if !c.is_control() => Key::Char(c),
                _ => Key::Other,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockTsfDocument {
        committed: String,
        preedit: String,
    }

    impl TsfContextBackend for MockTsfDocument {
        fn set_composition_text(&mut self, text: &str, _cursor_utf16: u32) {
            self.preedit = text.to_string();
        }
        fn end_composition_at_end(&mut self) {
            self.preedit.clear();
        }
        fn commit_text_at_selection(&mut self, text: &str) {
            self.committed.push_str(text);
            self.preedit.clear();
        }
        fn replace_before_caret(&mut self, delete_utf16: u32, insert: &str) {
            let mut u16_buf: Vec<u16> = self.committed.encode_utf16().collect();
            let keep = u16_buf.len().saturating_sub(delete_utf16 as usize);
            u16_buf.truncate(keep);
            self.committed = String::from_utf16(&u16_buf).unwrap_or_default();
            self.committed.push_str(insert);
        }
    }

    #[test]
    fn tsf_service_types_and_commits_vietnamese() {
        let mut tsf = VntypeTextService::new(Config::default());
        let mut doc = MockTsfDocument::default();
        tsf.on_set_focus("notepad", &[], &mut doc);

        for ch in "Vieejt".chars() {
            let before = doc.committed.clone();
            let eaten = tsf.on_key_down(KeyEvent::ch(ch), ch as u32, Some(&before), &mut doc);
            assert!(eaten);
        }
        assert_eq!(doc.preedit, "Việt");

        let before = doc.committed.clone();
        let eaten_space = tsf.on_key_down(KeyEvent::ch(' '), 0x20, Some(&before), &mut doc);
        assert!(eaten_space);
        assert_eq!(doc.committed, "Việt ");
        assert!(doc.preedit.is_empty());
    }
}
