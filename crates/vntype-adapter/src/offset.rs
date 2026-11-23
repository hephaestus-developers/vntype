//! Các hàm quy đổi chính xác giữa chỉ số Byte (UTF-8), ký tự Unicode (`char`) và đơn vị mã UTF-16.

/// Tính tổng độ dài của một chuỗi theo đơn vị mã UTF-16 (UTF-16 code units).
///
/// # Examples
///
/// ```
/// assert_eq!(vntype_adapter::utf16_len("tiếng"), 5);
/// assert_eq!(vntype_adapter::utf16_len("😀a"), 3);
/// ```
pub fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

/// Đổi chỉ số ký tự (`char` offset) sang chỉ số đơn vị mã UTF-16.
///
/// Trả về `None` nếu `chars` vượt quá tổng số ký tự của chuỗi.
pub fn char_to_utf16_offset(text: &str, chars: u32) -> Option<u32> {
    let mut count = 0u32;
    for (offset, ch) in text.chars().enumerate() {
        if offset as u32 == chars {
            return Some(count);
        }
        count += ch.len_utf16() as u32;
    }
    (chars as usize == text.chars().count()).then_some(count)
}

/// Đổi chỉ số đơn vị mã UTF-16 sang chỉ số ký tự (`char` offset).
///
/// Trả về `None` nếu vị trí cắt rơi vào giữa một cặp đại diện (surrogate pair) hoặc vượt quá độ dài chuỗi.
pub fn utf16_to_char_offset(text: &str, units: u32) -> Option<u32> {
    let mut used = 0u32;
    let mut chars = 0u32;
    for ch in text.chars() {
        if used == units {
            return Some(chars);
        }
        let width = ch.len_utf16() as u32;
        if used + width > units {
            return None;
        }
        used += width;
        chars += 1;
    }
    (used == units).then_some(chars)
}

/// Đổi chỉ số ký tự (`char` offset) sang chỉ số byte UTF-8.
pub fn char_to_byte_offset(text: &str, chars: u32) -> Option<usize> {
    let mut count = 0u32;
    for (offset, _) in text.char_indices() {
        if count == chars {
            return Some(offset);
        }
        count += 1;
    }
    (count == chars).then_some(text.len())
}

/// Đổi chỉ số đơn vị mã UTF-16 sang chỉ số byte UTF-8.
pub fn utf16_to_byte_offset(text: &str, units: u32) -> Option<usize> {
    let mut used = 0u32;
    for (offset, ch) in text.char_indices() {
        if used == units {
            return Some(offset);
        }
        let width = ch.len_utf16() as u32;
        if used + width > units {
            return None;
        }
        used += width;
    }
    (used == units).then_some(text.len())
}

/// Cắt phần tiền tố của chuỗi `text` có độ dài đúng bằng `units` đơn vị mã UTF-16.
pub fn utf16_prefix(text: &str, units: u32) -> Option<&str> {
    let offset = utf16_to_byte_offset(text, units)?;
    Some(&text[..offset])
}

/// Cắt phần hậu tố của chuỗi `text` có độ dài đúng bằng `units` đơn vị mã UTF-16.
pub fn utf16_suffix(text: &str, units: u32) -> Option<&str> {
    if units == 0 {
        return Some("");
    }
    let total = utf16_len(text);
    if units > total {
        return None;
    }
    let offset = utf16_to_byte_offset(text, total - units)?;
    Some(&text[offset..])
}

/// Cắt phần hậu tố của chuỗi `text` gồm đúng `chars` ký tự (`char`) trong thời gian $O(\text{chars})$.
pub fn char_suffix(text: &str, chars: u32) -> Option<&str> {
    if chars == 0 {
        return Some("");
    }
    let (byte_offset, _) = text.char_indices().rev().nth((chars - 1) as usize)?;
    Some(&text[byte_offset..])
}

/// Tính độ dài theo đơn vị mã UTF-16 của `chars` ký tự cuối cùng trong chuỗi `text`.
pub fn char_suffix_utf16_len(text: &str, chars: u32) -> Option<u32> {
    char_suffix(text, chars).map(utf16_len)
}

/// Lấy ký tự (`char`) nằm ngay tại vị trí con trỏ `cursor_utf16`.
pub fn next_codepoint(text: &str, cursor_utf16: u32) -> Option<char> {
    let offset = utf16_to_byte_offset(text, cursor_utf16)?;
    text[offset..].chars().next()
}

/// Kiểm tra một ký tự có được coi là thành phần của một từ (chữ cái, chữ số, gạch dưới, dấu tổ hợp) hay không.
pub fn is_word_codepoint(codepoint: char) -> bool {
    let cp = codepoint as u32;
    if cp < 0x80 {
        return codepoint.is_ascii_alphanumeric() || codepoint == '_';
    }
    (0x00C0..=0x024F).contains(&cp) || (0x0300..=0x036F).contains(&cp) || (0x1E00..=0x1EFF).contains(&cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_and_char_conversions_with_surrogate_pairs() {
        let s = "😀tiếng";
        assert_eq!(utf16_len(s), 7);
        assert_eq!(char_to_utf16_offset(s, 1), Some(2));
        assert_eq!(utf16_to_char_offset(s, 2), Some(1));
        assert_eq!(utf16_to_char_offset(s, 1), None); // giữa surrogate pair
        assert_eq!(utf16_to_byte_offset(s, 1), None);
        assert_eq!(utf16_prefix(s, 2), Some("😀"));
        assert_eq!(utf16_suffix(s, 5), Some("tiếng"));
        assert_eq!(utf16_suffix(s, 6), None);
        assert_eq!(char_suffix(s, 5), Some("tiếng"));
        assert_eq!(char_suffix_utf16_len(s, 5), Some(5));
        assert_eq!(next_codepoint(s, 2), Some('t'));
        assert!(is_word_codepoint('ế') && is_word_codepoint('_') && !is_word_codepoint(' '));
    }
}
