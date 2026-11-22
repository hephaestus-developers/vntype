//! Xử lý bảng gõ tắt (Macro) đơn từ và cụm nhiều từ, tự động bảo toàn kiểu chữ hoa/thường.

use std::collections::HashMap;

/// Kiểm tra một chuỗi có ít nhất một ký tự phân biệt hoa/thường và mọi ký tự đó đều viết hoa hay không.
///
/// # Examples
///
/// ```
/// use vntype_engine::py_isupper;
///
/// assert!(py_isupper("VIỆT"));
/// assert!(!py_isupper("Việt"));
/// assert!(!py_isupper("123"));
/// ```
pub fn py_isupper(s: &str) -> bool {
    let mut cased = false;
    for c in s.chars() {
        if c.is_lowercase() {
            return false;
        }
        if c.is_uppercase() {
            cased = true;
        }
    }
    cased
}

/// Biến đổi một từ đơn `word` qua bảng gõ tắt `macros`, tự động đồng bộ kiểu viết hoa:
/// - `"ko"` -> `"không"`
/// - `"Ko"` -> `"Không"`
/// - `"KO"` -> `"KHÔNG"`
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use vntype_engine::expand_single_macro;
///
/// let macros = HashMap::from([("ko".into(), "không".into())]);
/// assert_eq!(expand_single_macro(&macros, "Ko", "ko").as_deref(), Some("Không"));
/// assert_eq!(expand_single_macro(&macros, "KO", "ko").as_deref(), Some("KHÔNG"));
/// ```
pub fn expand_single_macro(macros: &HashMap<String, String>, matched: &str, key: &str) -> Option<String> {
    let m = macros.get(&key.to_lowercase())?;
    if m.is_empty() {
        return None;
    }
    if py_isupper(matched) && matched.chars().count() > 1 {
        return Some(m.to_uppercase());
    }
    if matched.chars().next().is_some_and(|c| c.is_uppercase()) {
        let mut it = m.chars();
        let first: String = it.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        return Some(first + it.as_str());
    }
    Some(m.clone())
}

/// Tìm kiếm cụm từ gõ tắt nhiều từ khớp dài nhất ở hậu tố của `phrase`.
///
/// Trả về bộ ba `(khóa_gõ_tắt, chuỗi_mở_rộng, tiền_tố_còn_lại)`.
pub fn match_phrase_macro(
    macros: &HashMap<String, String>,
    phrase: &str,
    current_word_len: usize,
) -> Option<(String, String, String)> {
    let low = phrase.to_lowercase();
    let mut best: Option<(usize, String, usize)> = None;
    for key in macros.keys() {
        let k = key.to_lowercase();
        let len = k.chars().count();
        if len == 0 || len <= current_word_len || !low.ends_with(&k) {
            continue;
        }
        let Some((split_byte, _)) = phrase.char_indices().rev().nth(len - 1) else {
            continue;
        };
        let prefix = &phrase[..split_byte];
        if let Some(last) = prefix.chars().last()
            && (last.is_alphanumeric() || last == '_' || matches!(last, '.' | '@' | '/' | ':' | '\\' | '-'))
        {
            continue;
        }
        if best.as_ref().is_none_or(|(best_len, _, _)| len > *best_len) {
            best = Some((len, key.clone(), split_byte));
        }
    }
    let (_, key, split_byte) = best?;
    let (prefix, matched) = phrase.split_at(split_byte);
    let expansion = expand_single_macro(macros, matched, &key)?;
    Some((key, expansion, prefix.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_macro_case_modes() {
        let macros = HashMap::from([("ko".into(), "không".into()), ("vn".into(), "việt nam".into())]);
        assert_eq!(expand_single_macro(&macros, "ko", "ko").as_deref(), Some("không"));
        assert_eq!(expand_single_macro(&macros, "Ko", "ko").as_deref(), Some("Không"));
        assert_eq!(expand_single_macro(&macros, "KO", "ko").as_deref(), Some("KHÔNG"));
        assert_eq!(expand_single_macro(&macros, "VN", "vn").as_deref(), Some("VIỆT NAM"));
        assert!(expand_single_macro(&macros, "xx", "xx").is_none());
    }

    #[test]
    fn phrase_macro_matches_unicode_prefix_safely() {
        let macros = HashMap::from([("chào bạn".into(), "xin chào bạn".into())]);
        let res = match_phrase_macro(&macros, "Tôi nói CHÀO BẠN", 3);
        assert_eq!(res, Some(("chào bạn".into(), "XIN CHÀO BẠN".into(), "Tôi nói ".into())));
        assert!(match_phrase_macro(&macros, "abcchào bạn", 3).is_none());
    }
}
