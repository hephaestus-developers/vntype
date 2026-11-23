//! Dịch sự kiện phím evdev/XKB và quản lý trạng thái các phím bổ trợ cho Wayland.

use vntype_engine::{Key, KeyEvent};

/// Cấu trúc biểu diễn một sự kiện phím từ `zwp_input_method_keyboard_grab_v2` đã qua dịch XKB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandKeyInput {
    /// Phím chuẩn hoá cho bộ gõ.
    pub key: Key,
    /// Mã phím vật lý evdev (dùng để chuyển tiếp qua bàn phím ảo `zwp_virtual_keyboard_v1`).
    pub keycode: u32,
    /// Dấu thời gian sự kiện (mili-giây).
    pub time: u32,
    /// `true` nếu là sự kiện nhấn phím, `false` nếu là sự kiện nhả phím.
    pub pressed: bool,
    /// Trạng thái phím Control đang giữ.
    pub ctrl: bool,
    /// Trạng thái phím Alt đang giữ.
    pub alt: bool,
    /// Trạng thái phím Super/Logo đang giữ.
    pub meta: bool,
}

impl WaylandKeyInput {
    /// Chuyển đổi sang [`KeyEvent`] của `vntype-engine`.
    pub fn to_key_event(&self) -> KeyEvent {
        KeyEvent { key: self.key.clone(), ctrl: self.ctrl, alt: self.alt, meta: self.meta }
    }
}

/// Ánh xạ mã keysym chuẩn của XKB sang [`Key`].
pub fn translate_xkb_keysym(keysym: u32, utf8_char: Option<char>) -> Key {
    match keysym {
        0xff08 => Key::Backspace,
        0xff0d | 0xff8d => Key::Enter,
        0xff1b => Key::Escape,
        0xff09 => Key::Tab,
        0xff51 => Key::Left,
        0xff53 => Key::Right,
        0xff52 => Key::Up,
        0xff54 => Key::Down,
        0xff50 => Key::Home,
        0xff57 => Key::End,
        0xffff => Key::Delete,
        0xff55 => Key::PageUp,
        0xff56 => Key::PageDown,
        0xffe1..=0xffee => Key::Modifier,
        _ => match utf8_char {
            Some(c) if !c.is_control() => Key::Char(c),
            _ => Key::Other,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xkb_keysym_and_event_translation() {
        assert_eq!(translate_xkb_keysym(0xff08, None), Key::Backspace);
        assert_eq!(translate_xkb_keysym(0xff0d, None), Key::Enter);
        assert_eq!(translate_xkb_keysym(b'a' as u32, Some('a')), Key::Char('a'));
        assert_eq!(translate_xkb_keysym(0, Some('\n')), Key::Other);

        let inp = WaylandKeyInput {
            key: Key::Char('a'),
            keycode: 30,
            time: 100,
            pressed: true,
            ctrl: true,
            alt: false,
            meta: false,
        };
        assert_eq!(
            inp.to_key_event(),
            KeyEvent { key: Key::Char('a'), ctrl: true, alt: false, meta: false }
        );
    }
}
