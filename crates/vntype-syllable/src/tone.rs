//! Quy tắc xác định vị trí đặt dấu thanh trên cụm nguyên âm tiếng Việt.

use alloc::string::String;
use alloc::vec::Vec;

use crate::phonology::{compose_char, decompose_char, parse, strip_tone, MARKED_VOWELS};

/// Quy tắc đặt dấu thanh cho các vần mở `oa`, `oe`, `uy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ToneStyle {
    /// Quy tắc cũ: đặt dấu trên nguyên âm đầu (`hòa`, `khỏe`, `thúy`).
    #[default]
    Old,
    /// Quy tắc mới: đặt dấu trên nguyên âm chính (`hoà`, `khoẻ`, `thuý`).
    New,
}

/// Tìm chỉ số (theo đơn vị `char`) của nguyên âm nhận dấu thanh trong một chuỗi chữ thường không thanh.
pub fn tone_index(s: &str, style: ToneStyle) -> Option<usize> {
    let syl = parse(s);
    let c: Vec<char> = syl.cluster.chars().collect();
    let st = syl.cluster_start;
    if c.is_empty() {
        return None;
    }
    if let Some(p) = c.iter().position(|&x| x == 'ơ') {
        return Some(st + p);
    }
    if let Some(p) = c.iter().position(|&x| MARKED_VOWELS.contains(x)) {
        return Some(st + p);
    }
    if !syl.coda.is_empty() {
        return Some(st + c.len() - 1);
    }
    if c.len() == 1 {
        return Some(st);
    }
    if c.len() == 3 {
        return Some(st + 1);
    }
    if matches!(syl.cluster.as_str(), "oa" | "oe" | "uy") {
        return Some(st + if style == ToneStyle::New { 1 } else { 0 });
    }
    Some(st)
}

/// Đặt (hoặc thay thế, hoặc xóa khi `tone == 0`) dấu thanh cho một từ tiếng Việt, bảo toàn dấu phụ và trạng thái viết hoa/thường.
pub fn place_tone(word: &str, tone: u8, style: ToneStyle) -> String {
    let (base, _) = strip_tone(word);
    if tone == 0 {
        return base;
    }
    let idx = match tone_index(&base.to_lowercase(), style) {
        Some(i) => i,
        None => return base,
    };
    base.chars()
        .enumerate()
        .map(|(i, ch)| {
            if i == idx {
                let d = decompose_char(ch);
                compose_char(d.base, d.mark, tone, d.upper)
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_placement_old_vs_new() {
        assert_eq!(place_tone("hoa", 2, ToneStyle::Old), "hòa");
        assert_eq!(place_tone("hoa", 2, ToneStyle::New), "hoà");
        assert_eq!(place_tone("thương", 1, ToneStyle::New), "thướng");
    }
}
