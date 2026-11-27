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
/// Mặt nạ phím Mod4 / Super trên X11/Wayland (`IBUS_MOD4_MASK`).
pub const IBUS_MOD4_MASK: u32 = 1 << 6;
/// Mặt nạ phím Mod5 / AltGr (`IBUS_MOD5_MASK`).
pub const IBUS_MOD5_MASK: u32 = 1 << 7;
/// Mặt nạ phím Super (`IBUS_SUPER_MASK`).
pub const IBUS_SUPER_MASK: u32 = 1 << 26;
/// Mặt nạ phím Hyper (`IBUS_HYPER_MASK`).
pub const IBUS_HYPER_MASK: u32 = 1 << 27;
/// Mặt nạ phím Meta (`IBUS_META_MASK`).
pub const IBUS_META_MASK: u32 = 1 << 28;
/// Mặt nạ đánh dấu sự kiện phím đã được xử lý (`IBUS_HANDLED_MASK`).
pub const IBUS_HANDLED_MASK: u32 = 1 << 24;
/// Mặt nạ đánh dấu sự kiện phím đã được engine chuyển tiếp (`IBUS_IGNORED_MASK`).
pub const IBUS_IGNORED_MASK: u32 = 1 << 25;
/// Tập mặt nạ các phím bổ trợ hệ thống (`Ctrl`, `Alt`, `AltGr`, `Super`, `Hyper`, `Meta`).
pub const IBUS_SYSTEM_MODIFIER_MASK: u32 = IBUS_CONTROL_MASK
    | IBUS_MOD1_MASK
    | IBUS_MOD4_MASK
    | IBUS_MOD5_MASK
    | IBUS_SUPER_MASK
    | IBUS_HYPER_MASK
    | IBUS_META_MASK;

/// Chuyển đổi cặp `(keyval, state)` từ giao thức IBus `ProcessKeyEvent` sang [`KeyEvent`].
///
/// Trả về `None` nếu là sự kiện nhả phím (`RELEASE_MASK`), phím bị bỏ qua (`IBUS_IGNORED_MASK`)
/// hoặc mã phím không ánh xạ được.
pub fn translate_ibus_key(keyval: u32, state: u32) -> Option<KeyEvent> {
    if (state & (IBUS_RELEASE_MASK | IBUS_HANDLED_MASK | IBUS_IGNORED_MASK)) != 0 {
        return None;
    }
    let ctrl = (state & IBUS_CONTROL_MASK) != 0;
    let alt = (state & (IBUS_MOD1_MASK | IBUS_MOD5_MASK)) != 0;
    let meta = (state & (IBUS_MOD4_MASK | IBUS_SUPER_MASK | IBUS_HYPER_MASK | IBUS_META_MASK)) != 0;

    let key = match keyval {
        0xff08 => Key::Backspace,
        0xff0d | 0xff8d => Key::Enter,
        0xff09 | 0xff89 => Key::Tab,
        0xff1b => Key::Escape,
        0xff51 | 0xff96 => Key::Left,
        0xff52 | 0xff97 => Key::Up,
        0xff53 | 0xff98 => Key::Right,
        0xff54 | 0xff99 => Key::Down,
        0xff50 | 0xff95 => Key::Home,
        0xff57 | 0xff9c => Key::End,
        0xffff | 0xff9f => Key::Delete,
        0xff55 | 0xff9a => Key::PageUp,
        0xff56 | 0xff9b => Key::PageDown,
        0xff80 => Key::Char(' '),
        0xffaa => Key::Char('*'),
        0xffab => Key::Char('+'),
        0xffac => Key::Char(','),
        0xffad => Key::Char('-'),
        0xffae => Key::Char('.'),
        0xffaf => Key::Char('/'),
        0xffb0..=0xffb9 => Key::Char(char::from_u32(b'0' as u32 + (keyval - 0xffb0))?),
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
        assert_eq!(translate_ibus_key(b'a' as u32, IBUS_HANDLED_MASK), None);
        assert_eq!(translate_ibus_key(b'a' as u32, IBUS_IGNORED_MASK), None);
        assert_eq!(
            translate_ibus_key(b'l' as u32, IBUS_MOD4_MASK),
            Some(KeyEvent { key: Key::Char('l'), ctrl: false, alt: false, meta: true })
        );
        assert_eq!(
            translate_ibus_key(b'e' as u32, IBUS_MOD5_MASK),
            Some(KeyEvent { key: Key::Char('e'), ctrl: false, alt: true, meta: false })
        );
        assert_eq!(translate_ibus_key(0xffb5, 0), Some(KeyEvent::ch('5')));
        assert_eq!(translate_ibus_key(0xffab, 0), Some(KeyEvent::ch('+')));
        assert_eq!(translate_ibus_key(0xff96, 0), Some(KeyEvent::new(Key::Left)));
        assert_eq!(translate_ibus_key(0xff9f, 0), Some(KeyEvent::new(Key::Delete)));
    }
}
