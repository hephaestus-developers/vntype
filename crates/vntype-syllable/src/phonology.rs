//! Phân tích cấu trúc âm tiết tiếng Việt (âm đầu, cụm nguyên âm, âm cuối) và tổ hợp ký tự.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::iter;
use unicode_normalization::UnicodeNormalization;

use crate::case::{lower, upper};

/// Danh sách toàn bộ nguyên âm tiếng Việt (không mang dấu thanh).
pub const VOWELS: &str = "aăâeêioôơuưy";

/// Danh sách các nguyên âm có dấu phụ (mũ, trăng, móc).
pub(crate) const MARKED_VOWELS: &str = "ăâêôơư";

/// Danh sách các nguyên âm hàng trước dùng để kiểm tra quy tắc kết hợp phụ âm đầu (`k/gh/ngh` và `c/g/ng`).
pub(crate) const FRONT_VOWELS: &str = "eêiy";

/// Danh sách các phụ âm đầu tiếng Việt, sắp xếp từ dài xuống ngắn để ưu tiên khớp chuỗi dài nhất.
pub(crate) const ONSETS: &[&str] = &[
    "ngh", "ng", "nh", "gh", "kh", "ph", "th", "tr", "ch", "b", "c", "d", "đ", "g", "h", "k", "l", "m",
    "n", "p", "r", "s", "t", "v", "x",
];

/// Dấu phụ tiếng Việt gắn trên nguyên âm hoặc chữ `d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mark {
    /// Dấu mũ trên `â`, `ê`, `ô`.
    Circumflex,
    /// Dấu trăng trên `ă`.
    Breve,
    /// Dấu móc trên `ơ`, `ư`.
    Horn,
    /// Dấu gạch ngang trên `đ`.
    Stroke,
}

impl Mark {
    /// Trả về ký tự dấu tổ hợp Unicode (Combining Character) tương ứng.
    ///
    /// Riêng `Mark::Stroke` trả về `None` vì chữ `đ` là một mã ký tự riêng biệt trong Unicode.
    pub fn combining(self) -> Option<char> {
        match self {
            Mark::Circumflex => Some('\u{0302}'),
            Mark::Breve => Some('\u{0306}'),
            Mark::Horn => Some('\u{031B}'),
            Mark::Stroke => None,
        }
    }

    /// Nhận diện `Mark` từ một ký tự dấu tổ hợp Unicode.
    pub(crate) fn from_combining(c: char) -> Option<Mark> {
        match c {
            '\u{0302}' => Some(Mark::Circumflex),
            '\u{0306}' => Some(Mark::Breve),
            '\u{031B}' => Some(Mark::Horn),
            _ => None,
        }
    }
}

/// Chuyển mã số dấu thanh (`1..=5`) sang ký tự dấu tổ hợp Unicode.
pub(crate) fn tone_combining(tone: u8) -> Option<char> {
    match tone {
        1 => Some('\u{0301}'), // Sắc
        2 => Some('\u{0300}'), // Huyền
        3 => Some('\u{0309}'), // Hỏi
        4 => Some('\u{0303}'), // Ngã
        5 => Some('\u{0323}'), // Nặng
        _ => None,
    }
}

/// Nhận diện mã số dấu thanh (`1..=5`) từ một ký tự dấu tổ hợp Unicode.
pub(crate) fn combining_tone(c: char) -> Option<u8> {
    match c {
        '\u{0301}' => Some(1),
        '\u{0300}' => Some(2),
        '\u{0309}' => Some(3),
        '\u{0303}' => Some(4),
        '\u{0323}' => Some(5),
        _ => None,
    }
}

/// Cấu trúc biểu diễn một âm tiết đã được tách thành các thành phần âm vị học.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syllable {
    /// Phụ âm đầu (ví dụ: `"ng"`, `"th"`, `"qu"`, `"gi"`).
    pub onset: String,
    /// Cụm nguyên âm ở giữa (ví dụ: `"uyê"`, `"ươ"`).
    pub cluster: String,
    /// Phụ âm cuối (ví dụ: `"ng"`, `"nh"`, `"ch"`, `"t"`).
    pub coda: String,
    /// Chỉ số theo đơn vị `char` nơi cụm nguyên âm bắt đầu.
    pub cluster_start: usize,
}

impl Syllable {
    /// Trả về chỉ số theo đơn vị `char` ngay sau khi kết thúc cụm nguyên âm.
    pub fn cluster_end(&self) -> usize {
        self.cluster_start + self.cluster.chars().count()
    }
}

/// Thông tin phân rã của một ký tự tiếng Việt đơn lẻ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decomposed {
    /// Ký tự chữ cái gốc ở dạng thường không dấu (`a`..`z`).
    pub base: char,
    /// Dấu phụ gắn kèm nếu có.
    pub mark: Option<Mark>,
    /// Dấu thanh từ 0 (ngang) đến 5 (nặng).
    pub tone: u8,
    /// Cờ cho biết ký tự ban đầu có phải chữ in hoa hay không.
    pub upper: bool,
}

/// Kiểm tra một ký tự chữ thường không dấu thanh có phải là nguyên âm tiếng Việt không.
#[inline]
pub fn is_vowel(c: char) -> bool {
    VOWELS.contains(c)
}

/// Kiểm tra một ký tự chữ thường không dấu thanh có thuộc bảng chữ cái tiếng Việt hoặc ASCII (`a..=z`) hay không.
#[inline]
pub fn is_letter(c: char) -> bool {
    c.is_ascii_lowercase() || c == 'đ' || is_vowel(c)
}

/// Ghép chữ cơ sở, dấu phụ, dấu thanh và trạng thái viết hoa thành một ký tự Unicode dựng sẵn (NFC).
pub fn compose_char(base: char, mark: Option<Mark>, tone: u8, is_upper: bool) -> char {
    let mut buf: [char; 3] = ['\0'; 3];
    let mut n = 0;
    match mark {
        Some(Mark::Stroke) => {
            buf[n] = if base == 'd' { 'đ' } else { base };
            n += 1;
        }
        Some(m) => {
            buf[n] = base;
            n += 1;
            if let Some(c) = m.combining() {
                buf[n] = c;
                n += 1;
            }
        }
        None => {
            buf[n] = base;
            n += 1;
        }
    }
    if let Some(t) = tone_combining(tone) {
        buf[n] = t;
        n += 1;
    }
    let mut it = buf[..n].iter().copied().nfc();
    let ch = it.next().unwrap_or(base);
    debug_assert!(it.next().is_none(), "compose_char không sinh ra đúng 1 ký tự: {:?}", &buf[..n]);
    if is_upper {
        upper(ch)
    } else {
        ch
    }
}

/// Phân rã một ký tự tiếng Việt bất kỳ thành chữ cơ sở, dấu phụ, dấu thanh và cờ viết hoa.
pub fn decompose_char(ch: char) -> Decomposed {
    let lo = lower(ch);
    let is_upper = ch != lo;
    if lo == 'đ' {
        return Decomposed { base: 'd', mark: Some(Mark::Stroke), tone: 0, upper: is_upper };
    }
    let mut it = iter::once(lo).nfd();
    let base = it.next().unwrap_or(lo);
    let mut mark = None;
    let mut tone = 0;
    for c in it {
        if let Some(m) = Mark::from_combining(c) {
            mark = Some(m);
        } else if let Some(t) = combining_tone(c) {
            tone = t;
        }
    }
    Decomposed { base, mark, tone, upper: is_upper }
}

/// Tách dấu thanh ra khỏi một từ, trả về cặp `(chuỗi_đã_bỏ_thanh, mã_thanh)`.
pub fn strip_tone(text: &str) -> (String, u8) {
    let mut out = String::with_capacity(text.len());
    let mut tone = 0;
    for ch in text.chars() {
        let d = decompose_char(ch);
        if d.tone != 0 {
            tone = d.tone;
        }
        out.push(compose_char(d.base, d.mark, 0, d.upper));
    }
    (out, tone)
}

/// Tách một chuỗi chữ thường không mang dấu thanh thành cấu trúc [`Syllable`].
pub fn parse(s: &str) -> Syllable {
    let cs: Vec<char> = s.chars().collect();
    let onset: &str = if s.starts_with("gi") && cs.len() > 2 && is_vowel(cs[2]) {
        "gi"
    } else if s.starts_with("qu") {
        "qu"
    } else {
        ONSETS.iter().copied().find(|o| s.starts_with(o)).unwrap_or("")
    };
    let i = onset.chars().count();
    let mut j = i;
    while j < cs.len() && is_vowel(cs[j]) {
        j += 1;
    }
    Syllable {
        onset: onset.to_string(),
        cluster: cs[i..j].iter().collect(),
        coda: cs[j..].iter().collect(),
        cluster_start: i,
    }
}
