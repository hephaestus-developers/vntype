//! Khôi phục chuỗi phím thô từ một từ tiếng Việt đã hiển thị trên màn hình.
//!
//! Mô-đun này phục vụ hai tính năng quan trọng của bộ gõ:
//! 1. Xoá theo ký tự hiển thị khi bấm phím Backspace (ví dụ: `"tiếng"` -> `"tiến"`).
//! 2. Nạp lại từ vừa gõ ngay trước con trỏ (`edit_previous_word`) để sửa dấu thanh hoặc dấu phụ.

use alloc::string::String;
use alloc::vec::Vec;
use vntype_syllable::{self as vs, Mark, upper};

use crate::composer::{TransformOptions, transform};
use crate::rules::InputMethodSpec;

/// Chuyển ngược một từ tiếng Việt sang chuỗi phím thô theo kiểu gõ `im` (ví dụ: `"tiếng"` -> `"tieengs"`).
///
/// Trả về `None` nếu từ chứa ký tự ngoài bảng chữ cái hoặc kiểu gõ không có phím tạo dấu tương ứng.
///
/// # Examples
///
/// ```
/// use vntype_core::{to_raw, TELEX};
///
/// assert_eq!(to_raw("tiếng", &TELEX).as_deref(), Some("tieengs"));
/// ```
pub fn to_raw(word: &str, im: &InputMethodSpec) -> Option<String> {
    let mut keys = String::new();
    let mut tone = 0u8;
    let mut prev: Option<(char, Option<Mark>)> = None;
    for ch in word.chars() {
        let d = vs::decompose_char(ch);
        let b = d.base;
        if !b.is_ascii_lowercase() {
            return None;
        }
        if d.tone != 0 {
            tone = d.tone;
        }
        let key_b = if d.upper { upper(b) } else { b };
        keys.push(key_b);
        if d.mark.is_none() && prev == Some((b, None)) && im.mark_key(b, Mark::Circumflex) == Some(b) {
            keys.push(key_b);
        }
        prev = Some((b, d.mark));
        if let Some(m) = d.mark {
            let mk = im.mark_key(b, m)?;
            keys.push(if d.upper && mk.is_alphabetic() { upper(mk) } else { mk });
        }
    }
    if tone != 0 {
        keys.push(im.tone_key(tone)?);
    }
    Some(keys)
}

/// Tìm và xác minh chuỗi phím thô có khả năng tái tạo chính xác từ `word` dưới cấu hình `opt`.
pub fn rebuild(word: &str, im: &InputMethodSpec, opt: &TransformOptions) -> Option<Vec<char>> {
    let raw: Vec<char> = to_raw(word, im)?.chars().collect();
    if transform(&raw, im, opt, false).text == word { Some(raw) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{TELEX, VNI};

    #[test]
    fn rebuild_roundtrip() {
        let opt = TransformOptions::default();
        assert_eq!(to_raw("tiếng", &TELEX).as_deref(), Some("tieengs"));
        assert_eq!(to_raw("Việt", &VNI).as_deref(), Some("Vie6t5"));
        assert!(rebuild("thương", &TELEX, &opt).is_some());
    }

    #[test]
    fn rebuild_uppercase_and_invalid() {
        let opt = TransformOptions::default();
        assert_eq!(to_raw("Đà", &TELEX).as_deref(), Some("DDaf"));
        assert!(rebuild("Đà", &TELEX, &opt).is_some());
        assert!(rebuild("Thương", &TELEX, &opt).is_some());
        assert!(to_raw("€", &TELEX).is_none());
        assert!(rebuild("€", &TELEX, &opt).is_none());
    }
}
