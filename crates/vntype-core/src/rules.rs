//! Định nghĩa bảng luật cho các phương pháp gõ tiếng Việt theo mẫu thiết kế Strategy.
//!
//! Việc bổ sung một kiểu gõ mới chỉ cần khai báo một hằng [`InputMethodSpec`] mới mà không cần
//! thay đổi thuật toán biến đổi trong [`crate::composer`].

use vntype_syllable::Mark;

/// Một quy tắc đặt dấu phụ lên nguyên âm hoặc phụ âm `d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkRule {
    /// Loại dấu phụ sẽ được áp dụng.
    pub mark: Mark,
    /// Chuỗi chứa các chữ cái cơ sở có thể nhận dấu này.
    pub targets: &'static str,
    /// Nếu `true`, quy tắc này áp dụng đồng thời cho cặp nguyên âm `"uo"` thành `"ươ"`.
    pub pair: bool,
}

const fn rule(mark: Mark, targets: &'static str) -> MarkRule {
    MarkRule { mark, targets, pair: false }
}

const fn pair(mark: Mark, targets: &'static str) -> MarkRule {
    MarkRule { mark, targets, pair: true }
}

/// Đặc tả toàn diện của một kiểu gõ tiếng Việt (Telex, Simple Telex, VNI, VIQR).
#[derive(Debug)]
pub struct InputMethodSpec {
    /// Tên định danh của kiểu gõ (ví dụ: `"telex"`, `"vni"`).
    pub name: &'static str,
    /// Bảng ánh xạ từ phím gõ sang mã dấu thanh (`1..=5`).
    pub tones: &'static [(char, u8)],
    /// Phím dùng để xoá dấu thanh hoặc dấu phụ hiện có.
    pub remove_tone: char,
    /// Bảng ánh xạ từ phím gõ sang danh sách các quy tắc dấu phụ theo thứ tự ưu tiên.
    pub marks: &'static [(char, &'static [MarkRule])],
    /// Các phím khi gõ đứng riêng sẽ sinh ra nguyên âm có dấu (ví dụ: Telex `w` -> `ư`).
    pub standalone: &'static [(char, (char, Mark))],
    /// Các phím ngoặc luôn chèn trực tiếp nguyên âm có dấu (`[` -> `ơ`, `]` -> `ư`).
    pub insert_keys: &'static [(char, (char, Mark))],
    /// Cờ cho biết các phím chữ số `0..9` có phải là phím chức năng tạo dấu hay không (dùng cho VNI).
    pub digit_keys: bool,
}

impl InputMethodSpec {
    /// Tra cứu mã dấu thanh (`1..=5`) tương ứng với phím `k`.
    pub fn tone_of(&self, k: char) -> Option<u8> {
        self.tones.iter().find(|(c, _)| *c == k).map(|(_, t)| *t)
    }

    /// Tra cứu danh sách quy tắc dấu phụ tương ứng với phím `k`.
    pub fn marks_for(&self, k: char) -> Option<&'static [MarkRule]> {
        self.marks.iter().find(|(c, _)| *c == k).map(|(_, r)| *r)
    }

    /// Tra cứu nguyên âm có dấu sinh ra khi phím `k` đứng riêng.
    pub fn standalone_for(&self, k: char) -> Option<(char, Mark)> {
        self.standalone.iter().find(|(c, _)| *c == k).map(|(_, v)| *v)
    }

    /// Tra cứu nguyên âm có dấu sinh ra từ phím ngoặc chèn trực tiếp `k`.
    pub fn insert_for(&self, k: char) -> Option<(char, Mark)> {
        self.insert_keys.iter().find(|(c, _)| *c == k).map(|(_, v)| *v)
    }

    /// Kiểm tra xem một ký tự có đóng vai trò là phím chức năng trong kiểu gõ này hay không.
    pub fn is_function_key(&self, k: char) -> bool {
        let k = vntype_syllable::lower(k);
        self.tone_of(k).is_some()
            || k == self.remove_tone
            || self.marks_for(k).is_some()
            || self.standalone_for(k).is_some()
            || self.insert_for(k).is_some()
    }

    /// Tìm phím tạo ra dấu phụ `mark` trên chữ cái cơ sở `base` (phục vụ khôi phục phím thô).
    pub fn mark_key(&self, base: char, mark: Mark) -> Option<char> {
        for (key, rules) in self.marks {
            for r in rules.iter() {
                if r.mark == mark && r.targets.contains(base) && !r.pair {
                    return Some(*key);
                }
            }
        }
        None
    }

    /// Tìm phím tạo ra dấu thanh `tone` (phục vụ khôi phục phím thô).
    pub fn tone_key(&self, tone: u8) -> Option<char> {
        self.tones.iter().find(|(_, t)| *t == tone).map(|(k, _)| *k)
    }

    /// Kiểm tra xem ký tự không phải chữ cái có được coi là một phần của từ đang gõ không (dùng cho VIQR).
    pub fn extra_word_key(&self, k: char) -> bool {
        let k = vntype_syllable::lower(k);
        if k.is_ascii_lowercase() {
            return false;
        }
        self.tone_of(k).is_some()
            || k == self.remove_tone
            || self.marks_for(k).is_some()
            || self.standalone_for(k).is_some()
    }

    /// Tra cứu đặc tả kiểu gõ theo tên định danh.
    pub fn by_name(name: &str) -> Option<&'static InputMethodSpec> {
        match name {
            "telex" => Some(&TELEX),
            "simple_telex" => Some(&SIMPLE_TELEX),
            "vni" => Some(&VNI),
            "viqr" => Some(&VIQR),
            _ => None,
        }
    }
}

const TELEX_TONES: &[(char, u8)] = &[('s', 1), ('f', 2), ('r', 3), ('x', 4), ('j', 5)];
const TELEX_A: &[MarkRule] = &[rule(Mark::Circumflex, "a")];
const TELEX_E: &[MarkRule] = &[rule(Mark::Circumflex, "e")];
const TELEX_O: &[MarkRule] = &[rule(Mark::Circumflex, "o")];
const TELEX_W: &[MarkRule] = &[pair(Mark::Horn, "uo"), rule(Mark::Horn, "uo"), rule(Mark::Breve, "a")];
const TELEX_D: &[MarkRule] = &[rule(Mark::Stroke, "d")];
const TELEX_MARKS: &[(char, &[MarkRule])] =
    &[('a', TELEX_A), ('e', TELEX_E), ('o', TELEX_O), ('w', TELEX_W), ('d', TELEX_D)];

/// Đặc tả kiểu gõ Telex chuẩn.
pub static TELEX: InputMethodSpec = InputMethodSpec {
    name: "telex",
    tones: TELEX_TONES,
    remove_tone: 'z',
    marks: TELEX_MARKS,
    standalone: &[('w', ('u', Mark::Horn))],
    insert_keys: &[('[', ('o', Mark::Horn)), (']', ('u', Mark::Horn))],
    digit_keys: false,
};

/// Đặc tả kiểu gõ Simple Telex (không tự biến `w` đứng riêng thành `ư`, không dùng `[` và `]`).
pub static SIMPLE_TELEX: InputMethodSpec = InputMethodSpec {
    name: "simple_telex",
    tones: TELEX_TONES,
    remove_tone: 'z',
    marks: TELEX_MARKS,
    standalone: &[],
    insert_keys: &[],
    digit_keys: false,
};

const VNI_6: &[MarkRule] = &[rule(Mark::Circumflex, "aeo")];
const VNI_7: &[MarkRule] = &[pair(Mark::Horn, "uo"), rule(Mark::Horn, "uo")];
const VNI_8: &[MarkRule] = &[rule(Mark::Breve, "a")];
const VNI_9: &[MarkRule] = &[rule(Mark::Stroke, "d")];

/// Đặc tả kiểu gõ VNI (dùng các phím số từ `0` đến `9`).
pub static VNI: InputMethodSpec = InputMethodSpec {
    name: "vni",
    tones: &[('1', 1), ('2', 2), ('3', 3), ('4', 4), ('5', 5)],
    remove_tone: '0',
    marks: &[('6', VNI_6), ('7', VNI_7), ('8', VNI_8), ('9', VNI_9)],
    standalone: &[],
    insert_keys: &[],
    digit_keys: true,
};

const VIQR_CIRCUMFLEX: &[MarkRule] = &[rule(Mark::Circumflex, "aeo")];
const VIQR_BREVE: &[MarkRule] = &[rule(Mark::Breve, "a")];
const VIQR_HORN: &[MarkRule] = &[pair(Mark::Horn, "uo"), rule(Mark::Horn, "uo")];
const VIQR_D: &[MarkRule] = &[rule(Mark::Stroke, "d")];

/// Đặc tả kiểu gõ VIQR (Vietnamese Quoted-Readable).
pub static VIQR: InputMethodSpec = InputMethodSpec {
    name: "viqr",
    tones: &[('\'', 1), ('`', 2), ('?', 3), ('~', 4), ('.', 5)],
    remove_tone: '0',
    marks: &[('^', VIQR_CIRCUMFLEX), ('(', VIQR_BREVE), ('+', VIQR_HORN), ('d', VIQR_D)],
    standalone: &[],
    insert_keys: &[],
    digit_keys: false,
};

/// Ánh xạ gõ tắt phụ âm đầu trong chế độ Quick Telex (`cc=ch`, `gg=gi`, `kk=kh`, `nn=ng`, `qq=qu`, `pp=ph`, `tt=th`).
pub fn quick_telex(c: char) -> Option<&'static str> {
    match c {
        'c' => Some("ch"),
        'g' => Some("gi"),
        'k' => Some("kh"),
        'n' => Some("ng"),
        'q' => Some("qu"),
        'p' => Some("ph"),
        't' => Some("th"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_lookup_and_predicates() {
        assert_eq!(InputMethodSpec::by_name("telex").map(|s| s.name), Some("telex"));
        assert_eq!(InputMethodSpec::by_name("simple_telex").map(|s| s.name), Some("simple_telex"));
        assert_eq!(InputMethodSpec::by_name("vni").map(|s| s.name), Some("vni"));
        assert_eq!(InputMethodSpec::by_name("viqr").map(|s| s.name), Some("viqr"));
        assert!(InputMethodSpec::by_name("unknown").is_none());

        assert!(TELEX.is_function_key('S'));
        assert!(TELEX.is_function_key('['));
        assert!(!SIMPLE_TELEX.is_function_key('['));
        assert!(VNI.digit_keys && !TELEX.digit_keys);

        assert!(VIQR.extra_word_key('\''));
        assert!(VIQR.extra_word_key('^'));
        assert!(!VIQR.extra_word_key('a'));
        assert!(!TELEX.extra_word_key('s'));
    }

    #[test]
    fn quick_telex_mapping() {
        assert_eq!(quick_telex('c'), Some("ch"));
        assert_eq!(quick_telex('g'), Some("gi"));
        assert_eq!(quick_telex('q'), Some("qu"));
        assert_eq!(quick_telex('a'), None);
    }
}
