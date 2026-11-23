//! Bộ chuyển đổi giữa 16 bảng mã tiếng Việt: Unicode NFC, NFD, TCVN3, VNI-Windows,
//! VISCII, VPS, BKHCM1, BKHCM2, Vietware-X, Vietware-Full, CP1258, VIQR, NCR và C-String.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use unicode_normalization::UnicodeNormalization;

use crate::charset_tables::STATIC_CHARSET_SPECS;

/// Danh sách toàn bộ các định danh bảng mã được hỗ trợ.
pub const CHARSETS: &[&str] = &[
    "unicode",
    "unicode_nfd",
    "tcvn3",
    "vni_windows",
    "cp1258",
    "viqr",
    "viscii",
    "vps",
    "bkhcm1",
    "bkhcm2",
    "vietware_x",
    "vietware_full",
    "ncr_decimal",
    "ncr_hex",
    "cstring_hex",
    "cstring_decimal",
    "utf8_literal",
    "x_utf8",
];

const SINGLE_BYTE: &[&str] =
    &["tcvn3", "vni_windows", "viscii", "vps", "bkhcm1", "bkhcm2", "vietware_x", "vietware_full"];

/// Trả về bộ lặp qua tất cả các tên định danh bảng mã được hỗ trợ.
pub fn charset_names() -> impl Iterator<Item = &'static str> {
    CHARSETS.iter().copied()
}

struct Table {
    display: &'static str,
    map: HashMap<char, &'static str>,
}

struct DecodeTable {
    chars: HashMap<&'static str, char>,
    starts: HashSet<char>,
    max_len: usize,
}

struct Tables {
    tables: HashMap<&'static str, Table>,
    decode: HashMap<&'static str, DecodeTable>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut tables = HashMap::with_capacity(STATIC_CHARSET_SPECS.len());
        let mut decode = HashMap::with_capacity(STATIC_CHARSET_SPECS.len());

        for spec in STATIC_CHARSET_SPECS {
            let mut map = HashMap::with_capacity(spec.mappings.len());
            let mut groups: HashMap<&'static str, Vec<char>> = HashMap::new();

            for &(ch, code) in spec.mappings {
                map.insert(ch, code);
                groups.entry(code).or_default().push(ch);
            }

            let mut chars = HashMap::with_capacity(groups.len());
            for (code, mut letters) in groups {
                if letters.len() == 1 {
                    chars.insert(code, letters[0]);
                } else if letters.len() == 2
                    && letters[0].to_lowercase().to_string() == letters[1].to_lowercase().to_string()
                {
                    letters.sort_by_key(|c| !c.is_lowercase());
                    chars.insert(code, letters[0]);
                }
            }

            if spec.key == "viscii" {
                for (code, ch) in [
                    ("\u{0002}", 'Ẳ'),
                    ("\u{0005}", 'Ẵ'),
                    ("\u{0006}", 'Ẫ'),
                    ("\u{0014}", 'Ỷ'),
                    ("\u{0019}", 'Ỹ'),
                    ("\u{001E}", 'Ỵ'),
                    ("\u{00A0}", 'Õ'),
                ] {
                    chars.insert(code, ch);
                }
            }

            let starts = chars.keys().filter_map(|k| k.chars().next()).collect();
            let max_len = chars.keys().map(|k| k.chars().count()).max().unwrap_or(0);
            decode.insert(spec.key, DecodeTable { chars, starts, max_len });
            tables.insert(spec.key, Table { display: spec.display_name, map });
        }

        Tables { tables, decode }
    })
}

/// Lấy tên hiển thị tiếng Việt của một bảng mã.
pub fn display_name(charset: &str) -> Option<&'static str> {
    match normalize(charset) {
        "unicode" => Some("Unicode (NFC)"),
        "unicode_nfd" => Some("Unicode tổ hợp (NFD)"),
        "utf8_literal" => Some("UTF-8 Literal"),
        "x_utf8" => Some("X UTF-8"),
        key => tables().tables.get(key).map(|t| t.display),
    }
}

/// Chuẩn hoá tên bảng mã người dùng nhập vào về tên định danh chuẩn.
pub fn normalize(name: &str) -> &'static str {
    match name.trim().to_lowercase().as_str() {
        "unicode" | "nfc" | "utf-8" | "utf8" => "unicode",
        "unicode_nfd" | "unicode tổ hợp" | "nfd" | "decomposed" => "unicode_nfd",
        "tcvn3" | "tcvn3 (abc)" | "abc" => "tcvn3",
        "vni" | "vni windows" | "vni_windows" => "vni_windows",
        "cp1258" | "windows 1258 codepage" | "windows-1258" => "cp1258",
        "viqr" => "viqr",
        "viscii" => "viscii",
        "vps" => "vps",
        "bkhcm1" | "bkhcm 1" => "bkhcm1",
        "bkhcm2" | "bkhcm 2" => "bkhcm2",
        "vietware_x" | "vietware x" => "vietware_x",
        "vietware_full" | "vietware full" => "vietware_full",
        "ncr_decimal" | "ncr decimal" | "ncr" => "ncr_decimal",
        "ncr_hex" | "ncr hex" => "ncr_hex",
        "cstring_hex" | "unicode c string hex" | "unicode c string" => "cstring_hex",
        "cstring_decimal" | "unicode c string decimal" => "cstring_decimal",
        "utf8_literal" | "utf-8 literal" => "utf8_literal",
        "x_utf8" | "x utf-8" => "x_utf8",
        _ => "unicode",
    }
}

/// Kiểm tra bảng mã có phải là bảng mã đơn byte (1-byte legacy encoding) hay không.
pub fn is_single_byte(charset: &str) -> bool {
    SINGLE_BYTE.contains(&normalize(charset))
}

fn is_ascii_punct(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

/// Mã hoá văn bản từ Unicode sang bảng mã đích `charset`.
pub fn encode(text: &str, charset: &str) -> String {
    let key = normalize(charset);
    match key {
        "unicode" => text.nfc().collect(),
        "unicode_nfd" => text.nfd().collect(),
        "utf8_literal" => encode_utf8_literal(text),
        "x_utf8" => encode_x_utf8(text),
        "viqr" => encode_viqr(text),
        _ => {
            let Some(table) = tables().tables.get(key) else { return text.to_string() };
            let mut out = String::with_capacity(text.len());
            for ch in text.chars() {
                match table.map.get(&ch) {
                    Some(code) => out.push_str(code),
                    None => out.push(ch),
                }
            }
            out
        }
    }
}

fn encode_utf8_literal(text: &str) -> String {
    let normalized: String = text.nfc().collect();
    let mut out = String::with_capacity(normalized.len() * 2);
    let mut buf = [0u8; 4];
    for ch in normalized.chars() {
        if ch.is_ascii() {
            out.push(ch);
        } else {
            for &b in ch.encode_utf8(&mut buf).as_bytes() {
                out.push(b as char);
            }
        }
    }
    out
}

fn decode_utf8_literal(text: &str) -> String {
    let mut bytes = Vec::with_capacity(text.len());
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        if let Ok(b) = u8::try_from(ch as u32) {
            bytes.push(b);
        } else {
            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).nfc().collect()
}

fn encode_x_utf8(text: &str) -> String {
    use core::fmt::Write;
    let normalized: String = text.nfc().collect();
    let mut out = String::with_capacity(normalized.len() * 4);
    let mut buf = [0u8; 4];
    for ch in normalized.chars() {
        if ch.is_ascii() {
            out.push(ch);
        } else {
            for &b in ch.encode_utf8(&mut buf).as_bytes() {
                let _ = write!(out, "\\x{b:02x}");
            }
        }
    }
    out
}

fn decode_x_utf8(text: &str) -> String {
    let raw = text.as_bytes();
    let mut bytes = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\\'
            && i + 3 < raw.len()
            && (raw[i + 1] == b'x' || raw[i + 1] == b'X')
            && let Ok(hex) = core::str::from_utf8(&raw[i + 2..i + 4])
            && let Ok(val) = u8::from_str_radix(hex, 16)
        {
            bytes.push(val);
            i += 4;
            continue;
        }
        bytes.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).nfc().collect()
}

fn encode_viqr(text: &str) -> String {
    let Some(table) = tables().tables.get("viqr") else { return text.to_string() };
    let codes: HashSet<&str> = table.map.values().copied().collect();
    let mut out = String::with_capacity(text.len());
    let mut prev = String::new();
    for ch in text.chars() {
        let found = table.map.get(&ch).copied();
        let joined = format!("{prev}{ch}");
        let escaped = ch == '\\' || (is_ascii_punct(ch) && codes.contains(joined.as_str()));
        if found.is_some() || escaped {
            if escaped {
                out.push('\\');
            }
            match found {
                Some(code) => out.push_str(code),
                None => out.push(ch),
            }
        } else {
            out.push(ch);
        }
        if escaped {
            prev.clear();
        } else {
            prev = found.unwrap_or("").to_string();
            if found.is_none() {
                prev.clear();
                prev.push(ch);
            }
        }
    }
    out
}

/// Giải mã văn bản từ bảng mã `charset` về chuẩn Unicode dựng sẵn (NFC).
pub fn decode(text: &str, charset: &str) -> String {
    let key = normalize(charset);
    match key {
        "unicode" | "unicode_nfd" => return text.nfc().collect(),
        "utf8_literal" => return decode_utf8_literal(text),
        "x_utf8" => return decode_x_utf8(text),
        _ => {}
    }
    let Some(table) = tables().decode.get(key) else { return text.nfc().collect() };
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if key == "viqr" && chars[i] == '\\' && i + 1 < chars.len() && is_ascii_punct(chars[i + 1]) {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        let ch = chars[i];
        let mut found: Option<(char, usize)> = None;
        if table.starts.contains(&ch) {
            let max = table.max_len.min(chars.len() - i);
            for n in (1..=max).rev() {
                let code: String = chars[i..i + n].iter().collect();
                if let Some(&c) = table.chars.get(code.as_str()) {
                    found = Some((c, n));
                    break;
                }
            }
        }
        match found {
            Some((c, n)) if c != ch || n > 1 => {
                out.push(c);
                i += n;
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out.nfc().collect()
}

/// Giải mã mảng byte thô từ bảng mã một byte sang chuỗi Unicode NFC.
pub fn decode_bytes(data: &[u8], charset: &str) -> String {
    let text: String = data.iter().map(|&b| b as char).collect();
    decode(&text, charset)
}

/// Mã hoá chuỗi Unicode sang mảng byte thô cho các bảng mã một byte (như TCVN3, VISCII).
pub fn encode_bytes(text: &str, charset: &str) -> Option<Vec<u8>> {
    if !is_single_byte(charset) {
        return None;
    }
    encode(text, charset).chars().map(|c| u8::try_from(c as u32).ok()).collect()
}

/// Chuyển đổi văn bản trực tiếp từ bảng mã nguồn `from` sang bảng mã đích `to`.
///
/// # Examples
///
/// ```
/// assert_eq!(vntype_core::convert_charset("Tiếng Việt", "unicode", "viqr"), "Tie^'ng Vie^.t");
/// ```
pub fn convert(text: &str, from: &str, to: &str) -> String {
    encode(&decode(text, from), to)
}

const DETECT_PRIORITY: &[&str] = &[
    "tcvn3",
    "vni_windows",
    "viscii",
    "vps",
    "bkhcm1",
    "bkhcm2",
    "vietware_x",
    "vietware_full",
    "cp1258",
    "viqr",
    "ncr_hex",
    "ncr_decimal",
    "cstring_hex",
    "cstring_decimal",
];

/// Tự động nhận diện các bảng mã có khả năng cao nhất của chuỗi đầu vào.
pub fn detect(text: &str) -> Vec<&'static str> {
    let mut candidates: Vec<&str> =
        CHARSETS.iter().copied().filter(|c| *c != "unicode" && *c != "unicode_nfd").collect();
    if text.to_lowercase().contains("&#x") {
        candidates.sort_by_key(|c| *c != "ncr_hex");
    } else if text.contains("&#") {
        candidates.sort_by_key(|c| *c != "ncr_decimal");
    } else if text.contains("\\u") || text.contains("\\x") {
        candidates.sort_by_key(|c| *c != "cstring_hex" && *c != "cstring_decimal");
    }
    let mut scored: Vec<(usize, &'static str)> = candidates
        .into_iter()
        .filter_map(|key| {
            let decoded = decode(text, key);
            if decoded == text {
                return None;
            }
            let score = decoded
                .chars()
                .filter(|c| !c.is_ascii_alphanumeric() && !c.is_whitespace() && !c.is_ascii())
                .count();
            Some((score, key))
        })
        .collect();
    scored.sort_by_key(|(score, key)| {
        let prio = DETECT_PRIORITY.iter().position(|p| p == key).unwrap_or(999);
        (std::cmp::Reverse(*score), prio)
    });
    scored.into_iter().map(|(_, key)| key).collect()
}

/// Đổi kiểu chữ hoa, chữ thường hoặc viết hoa đầu từ (`"upper"`, `"lower"`, `"title"`).
pub fn change_case(text: &str, mode: &str) -> Option<String> {
    match mode.to_lowercase().as_str() {
        "upper" => Some(text.to_uppercase()),
        "lower" => Some(text.to_lowercase()),
        "title" => Some(
            text.split_inclusive(' ')
                .map(|part| {
                    let mut c = part.chars();
                    match c.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Loại bỏ toàn bộ dấu thanh và dấu phụ tiếng Việt khỏi văn bản, trả về chữ cái Latin không dấu.
pub fn remove_diacritics(text: &str) -> String {
    text.chars()
        .flat_map(|ch| ch.to_string().nfd().collect::<Vec<_>>())
        .filter(|ch| unicode_normalization::char::canonical_combining_class(*ch) == 0)
        .map(|ch| match ch {
            'đ' => 'd',
            'Đ' => 'D',
            c => c,
        })
        .collect::<String>()
        .nfc()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_tables_viqr_and_tcvn3_roundtrip() {
        assert_eq!(encode("tiếng Việt", "viqr"), "tie^'ng Vie^.t");
        assert_eq!(decode("tie^'ng Vie^.t", "viqr"), "tiếng Việt");
        assert_eq!(encode("mưa.", "viqr"), "mu+a\\.");
        assert_eq!(decode("mu+a\\.", "viqr"), "mưa.");
        assert_eq!(encode_bytes("Tiếng Việt", "tcvn3").unwrap(), b"Ti\xd5ng Vi\xd6t");
        assert_eq!(decode_bytes(b"Ti\xd5ng Vi\xd6t", "tcvn3"), "Tiếng Việt");
    }

    #[test]
    fn static_tables_all_16_charsets_roundtrip() {
        for key in CHARSETS {
            let encoded = encode("Tiếng Việt: Đắk Lắk, 2026!", key);
            assert_eq!(decode(&encoded, key), "Tiếng Việt: Đắk Lắk, 2026!", "Lỗi tại bảng mã: {key}");
        }
    }

    #[test]
    fn charset_helpers_and_transforms() {
        assert_eq!(normalize("NFC"), "unicode");
        assert_eq!(normalize("ABC"), "tcvn3");
        assert_eq!(display_name("unicode"), Some("Unicode (NFC)"));
        assert!(is_single_byte("tcvn3") && !is_single_byte("unicode"));
        assert!(encode_bytes("Tiếng", "unicode").is_none());

        assert_eq!(convert("Tiếng Việt", "unicode", "viqr"), "Tie^'ng Vie^.t");
        assert!(detect("tie^'ng Vie^.t").contains(&"viqr"));
        assert_eq!(detect("&#x1EBF;").first().copied(), Some("ncr_hex"));

        assert_eq!(change_case("tiếng việt", "upper").as_deref(), Some("TIẾNG VIỆT"));
        assert_eq!(change_case("TIẾNG VIỆT", "lower").as_deref(), Some("tiếng việt"));
        assert_eq!(change_case("tiếng việt", "title").as_deref(), Some("Tiếng Việt"));
        assert!(change_case("abc", "unknown").is_none());

        assert_eq!(remove_diacritics("Tiếng Việt Đắk Lắk"), "Tieng Viet Dak Lak");
    }
}
