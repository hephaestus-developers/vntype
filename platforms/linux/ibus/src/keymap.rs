//! Ánh xạ mã phím `keyval` và cờ trạng thái (`state`) của IBus sang [`vntype_engine::KeyEvent`].

use vntype_engine::{Key, KeyEvent};

/// Mặt nạ cờ sự kiện nhả phím trong IBus (`IBUS_RELEASE_MASK`).
pub const IBUS_RELEASE_MASK: u32 = 1 << 30;
/// Mặt nạ phím Shift (`IBUS_SHIFT_MASK`).
pub const IBUS_SHIFT_MASK: u32 = 1 << 0;
/// Mặt nạ phím Control (`IBUS_CONTROL_MASK`).
pub const IBUS_CONTROL_MASK: u32 = 1 << 2;
/// Mặt nạ phím Alt / Mod1 (`IBUS_MOD1_MASK`).
pub const IBUS_MOD1_MASK: u32 = 1 << 3;
/// Mặt nạ phím Super (`IBUS_SUPER_MASK`).
pub const IBUS_SUPER_MASK: u32 = 1 << 26;
/// Mặt nạ phím Meta (`IBUS_META_MASK`).
pub const IBUS_META_MASK: u32 = 1 << 28;
/// Mặt nạ đánh dấu sự kiện phím đã được engine chuyển tiếp (`IBUS_IGNORED_MASK`).
pub const IBUS_IGNORED_MASK: u32 = 1 << 25;

/// Chuyển đổi cặp `(keyval, state)` từ giao thức IBus `ProcessKeyEvent` sang [`KeyEvent`].
///
/// Trả về `None` nếu là sự kiện nhả phím (`RELEASE_MASK`), phím bị bỏ qua (`IBUS_IGNORED_MASK`)
/// hoặc mã phím không ánh xạ được.
pub fn translate_ibus_key(keyval: u32, state: u32) -> Option<KeyEvent> {
    if (state & (IBUS_RELEASE_MASK | IBUS_IGNORED_MASK)) != 0 {
        return None;
    }
    let ctrl = (state & IBUS_CONTROL_MASK) != 0;
    let alt = (state & IBUS_MOD1_MASK) != 0;
    let meta = (state & (IBUS_SUPER_MASK | IBUS_META_MASK)) != 0;

    let key = match keyval {
        0xff08 => Key::Backspace,
        0xff0d | 0xff8d => Key::Enter,
        0xff09 => Key::Tab,
        0xff1b => Key::Escape,
        0xff51 => Key::Left,
        0xff52 => Key::Up,
        0xff53 => Key::Right,
        0xff54 => Key::Down,
        0xff50 => Key::Home,
        0xff57 => Key::End,
        0xffff => Key::Delete,
        0xff55 => Key::PageUp,
        0xff56 => Key::PageDown,
        0xffe1..=0xffee => Key::Modifier,
        0x0020..=0x007e => Key::Char(char::from_u32(keyval)?),
        0x0100_0000..=0x0110_ffff => {
            let ch = char::from_u32(keyval & 0x00ff_ffff)?;
            if ch.is_control() {
                return None;
            }
            Key::Char(ch)
        }
        _ => {
            let ch = char::from_u32(keyval)?;
            if !ch.is_control() && keyval < 0xf000 {
                Key::Char(ch)
            } else {
                return None;
            }
        }
    };

    Some(KeyEvent { key, ctrl, alt, meta })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_basic_and_unicode_ibus_keys() {
        assert_eq!(translate_ibus_key(b'a' as u32, 0), Some(KeyEvent::ch('a')));
        assert_eq!(translate_ibus_key(0xff08, 0), Some(KeyEvent::new(Key::Backspace)));
        assert_eq!(translate_ibus_key(0x0100_0111, 0), Some(KeyEvent::ch('đ')));
        assert_eq!(translate_ibus_key(b'a' as u32, IBUS_RELEASE_MASK), None);
        assert_eq!(translate_ibus_key(b'a' as u32, IBUS_IGNORED_MASK), None);
    }
}
