//! Máy trạng thái biến đổi chuỗi phím thô sang chữ tiếng Việt hiển thị (`Composer`).
//!
//! Thuật toán hoàn toàn tất định, không trạng thái toàn cục và không cấp phát heap cho mảng chữ cái
//! của từ đang gõ nhờ cấu trúc [`ArrayVec`].

use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use arrayvec::ArrayVec;
use core::cmp::Reverse;
use vntype_syllable::{self as vs, Mark, Status, ToneStyle, lower};

use crate::rules::{InputMethodSpec, MarkRule, quick_telex};

/// Độ dài chuỗi phím thô tối đa cho một từ; vượt ngưỡng này bộ gõ coi là chuỗi mã/URL và giữ nguyên.
pub const MAX_RAW: usize = 24;

/// Số lượng phần tử [`Letter`] tối đa trong một từ (Quick Telex có thể mở rộng 1 phím thành 2 chữ cái).
pub const MAX_LETTERS: usize = 32;

const OPEN_UO_ONSETS: &[&str] = &["", "c", "h", "k", "kh", "qu", "th"];

/// Các tuỳ chọn điều khiển thuật toán biến đổi âm tiết của [`Composer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformOptions {
    /// Quy tắc đặt dấu thanh cũ hoặc mới.
    pub tone_style: ToneStyle,
    /// Bật kiểm tra chính tả để tự động khôi phục phím thô khi từ không phải tiếng Việt hợp lệ.
    pub spell_check: bool,
    /// Cho phép gõ tự do (không chặn đặt dấu khi từ chưa hợp lệ).
    pub free_typing: bool,
    /// Tự động khôi phục phím thô với từ sai chính tả.
    pub auto_restore_keys: bool,
    /// Cho phép phím `w` đứng riêng biến thành `ư` trong Telex.
    pub w_as_u: bool,
    /// Cho phép phím `[` và `]` biến thành `ơ` và `ư` trong Telex.
    pub bracket_keys: bool,
    /// Cho phép gõ tắt phụ âm đầu (`cc=ch`, `gg=gi`, ...).
    pub quick_telex: bool,
    /// Cho phép phụ âm cuối `k` với thanh sắc và nặng cho tên riêng (`Đắk Lắk`).
    pub allow_coda_k: bool,
}

impl Default for TransformOptions {
    fn default() -> Self {
        Self {
            tone_style: ToneStyle::Old,
            spell_check: true,
            free_typing: false,
            auto_restore_keys: true,
            w_as_u: true,
            bracket_keys: false,
            quick_telex: false,
            allow_coda_k: false,
        }
    }
}

/// Biểu diễn một vị trí chữ cái hiển thị trong từ đang được soạn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Letter {
    /// Chữ cái cơ sở ở dạng thường không dấu.
    pub base: char,
    /// Cờ cho biết chữ cái này có viết hoa hay không.
    pub upper: bool,
    /// Dấu phụ gắn trên chữ cái này nếu có.
    pub mark: Option<Mark>,
    /// Đánh dấu chữ cái này sinh ra từ phím `w` đứng riêng hoặc phím ngoặc `[/]`.
    pub from_standalone: bool,
    /// Đánh dấu dấu móc trên `u` được tự động bật bởi quy tắc cặp `ươ`.
    pub auto_horn: bool,
}

impl Letter {
    /// Tạo một chữ cái cơ sở mới chưa có dấu phụ.
    pub fn new(base: char, upper: bool) -> Self {
        Letter { base, upper, mark: None, from_standalone: false, auto_horn: false }
    }

    /// Tạo một chữ cái giữ nguyên theo ký tự phím gõ vào.
    pub fn literal(k: char) -> Self {
        let lk = lower(k);
        Letter::new(lk, k != lk)
    }
}

/// Trạng thái tích luỹ của một từ sau khi xử lý một chuỗi phím.
#[derive(Debug, Clone, Default)]
pub struct WordState {
    /// Danh sách các chữ cái hiển thị của từ hiện tại.
    pub letters: ArrayVec<Letter, MAX_LETTERS>,
    /// Mã dấu thanh hiện tại của toàn từ (`0..=5`).
    pub tone: u8,
    /// Cờ ghi nhận người dùng đã chủ động gõ phím lặp để hoàn tác dấu ít nhất một lần.
    pub undone: bool,
    /// Cờ khoá toàn bộ biến đổi thanh cho các phím tiếp theo sau khi hoàn tác thanh.
    pub latched: bool,
    /// Danh sách các phím dấu phụ đã bị khoá sau khi người dùng hoàn tác dấu phụ đó.
    pub locked: Vec<char>,
    /// Cờ ghi nhận từ này đã từng trải qua ít nhất một phép biến đổi tiếng Việt.
    pub transformed: bool,
    /// Phím gây ra thao tác hoàn tác gần nhất.
    pub undo_key: Option<char>,
    /// Cờ bảo vệ chữ `đ` viết tắt (như `đc`, `vcđ`) không bị bộ kiểm tra chính tả khôi phục về thô.
    pub keep: bool,
    /// Chỉ số vị trí của chữ `đ` được bảo vệ.
    pub keep_pos: usize,
    /// Phím thô ngay trước đó để phát hiện gõ lặp liền kề.
    pub last_key: Option<char>,
}

/// Kết quả trả về sau khi biến đổi một chuỗi phím thô.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformResult {
    /// Chuỗi chữ hiển thị kết quả (ở dạng Unicode NFC).
    pub text: String,
    /// `true` nếu từ bị khôi phục về chuỗi phím thô do vi phạm chính tả.
    pub restored: bool,
    /// Trạng thái chính tả của kết quả.
    pub status: Status,
    /// Chuỗi kết quả khi vừa hoàn tác bằng gõ lặp và khoá biến đổi (`undone && latched`); rỗng trong các trường hợp còn lại.
    pub rendered: String,
}

type Disp = (char, Option<Mark>, bool);

fn display(letters: &[Letter]) -> Vec<Disp> {
    let mut out: Vec<Disp> = letters.iter().map(|l| (l.base, l.mark, l.upper)).collect();
    let n = out.len();
    for i in 0..n.saturating_sub(1) {
        if out[i].0 == 'u' && out[i].1 == Some(Mark::Horn) && out[i + 1].0 == 'o' && out[i + 1].1.is_none()
        {
            out[i + 1] = ('o', Some(Mark::Horn), out[i + 1].2);
        }
    }
    if n >= 2
        && out[n - 2].0 == 'u'
        && out[n - 2].1 == Some(Mark::Horn)
        && out[n - 1].0 == 'o'
        && out[n - 1].1 == Some(Mark::Horn)
        && letters[n - 2].auto_horn
    {
        out[n - 2] = ('u', None, out[n - 2].2);
    }
    out
}

fn toneless_lower(letters: &[Letter]) -> String {
    display(letters).into_iter().map(|(b, m, _)| vs::compose_char(b, m, 0, false)).collect()
}

fn render(st: &WordState, style: ToneStyle) -> String {
    let disp = display(&st.letters);
    let mut chars: Vec<char> = disp.iter().map(|&(b, m, up)| vs::compose_char(b, m, 0, up)).collect();
    if st.tone != 0 {
        let lowered: String = disp.iter().map(|&(b, m, _)| vs::compose_char(b, m, 0, false)).collect();
        let idx = vs::tone_index(&lowered, style)
            .or_else(|| (0..disp.len()).rev().find(|&i| vs::is_vowel(disp[i].0)));
        if let Some(i) = idx {
            let (b, m, up) = disp[i];
            chars[i] = vs::compose_char(b, m, st.tone, up);
        }
    }
    chars.into_iter().collect()
}

fn status(letters: &[Letter], tone: u8, opt: &TransformOptions) -> Status {
    vs::check_with(&toneless_lower(letters), tone, opt.allow_coda_k)
}

fn status_for(letters: &[Letter], tone: u8, opt: &TransformOptions, is_final: bool) -> Status {
    if is_final {
        vs::check_final(&toneless_lower(letters), tone, opt.allow_coda_k)
    } else {
        status(letters, tone, opt)
    }
}

fn vni_literal(raw: &[char], im: &InputMethodSpec) -> Option<String> {
    if !im.digit_keys || !raw.iter().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let mut changed = false;
    while i < raw.len() {
        if i + 1 < raw.len() && raw[i].is_ascii_digit() && raw[i] == raw[i + 1] {
            out.push(raw[i]);
            i += 2;
            changed = true;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    if changed {
        return Some(out);
    }
    let mut tones = 0;
    for &c in raw.iter().rev() {
        if !c.is_ascii_digit() {
            break;
        }
        if ('0'..='5').contains(&c) {
            tones += 1;
        }
    }
    if tones >= 2 { Some(raw.iter().collect()) } else { None }
}

fn cluster_span(letters: &[Letter]) -> (usize, usize) {
    let syl = vs::parse(&toneless_lower(letters));
    (syl.cluster_start, syl.cluster_end())
}

type Cand = Vec<(usize, Mark)>;

fn candidates(st: &WordState, rules: &[MarkRule]) -> Vec<Cand> {
    let (start, end) = cluster_span(&st.letters);
    let l = &st.letters;
    let mut pairs: Vec<Cand> = Vec::new();
    let mut singles: Vec<Cand> = Vec::new();
    for r in rules {
        if r.mark == Mark::Stroke {
            if !l.is_empty() && l[0].base == 'd' && end > start {
                singles.push(vec![(0, Mark::Stroke)]);
            } else if l.last().is_some_and(|x| x.base == 'd') {
                singles.push(vec![(l.len() - 1, Mark::Stroke)]);
            }
        } else if r.pair {
            for i in start..end.saturating_sub(1) {
                if l[i].base == 'u' && l[i + 1].base == 'o' {
                    pairs.push(vec![(i, Mark::Horn), (i + 1, Mark::Horn)]);
                }
            }
        } else {
            for (i, letter) in l.iter().enumerate().take(end).skip(start) {
                if r.targets.contains(letter.base) {
                    singles.push(vec![(i, r.mark)]);
                }
            }
        }
    }
    singles.sort_by_key(|c| Reverse(c[0].0));
    let mut out: Vec<Cand> = Vec::new();
    for c in pairs.into_iter().chain(singles) {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

fn is_applied(l: &[Letter], c: &[(usize, Mark)]) -> bool {
    c.iter().all(|&(i, m)| l[i].mark == Some(m))
}

fn step(st: &mut WordState, k: char, im: &InputMethodSpec, opt: &TransformOptions) {
    let lk = lower(k);
    let prev = st.last_key;
    st.last_key = Some(lk);
    if st.latched || st.locked.contains(&lk) {
        let _ = st.letters.try_push(Letter::literal(k));
        return;
    }
    let (start, end) = cluster_span(&st.letters);
    let has_vowel =
        end > start || (st.letters.len() == 2 && st.letters[0].base == 'q' && st.letters[1].base == 'u');

    if opt.quick_telex
        && st.letters.len() == 1
        && !st.transformed
        && st.letters[0].base == lk
        && st.letters[0].mark.is_none()
        && let Some(q) = quick_telex(lk)
    {
        let up = st.letters[0].upper;
        let k_upper = k.is_uppercase();
        st.letters =
            q.chars().enumerate().map(|(i, c)| Letter::new(c, up && (i == 0 || k_upper))).collect();
        st.transformed = true;
        return;
    }

    if let Some(t) = im.tone_of(lk)
        && has_vowel
    {
        if st.tone == t {
            if prev == Some(lk) {
                st.tone = 0;
                st.undone = true;
                st.undo_key = Some(lk);
                st.latched = true;
                let _ = st.letters.try_push(Letter::literal(k));
            }
            return;
        }
        let lowered = toneless_lower(&st.letters);
        if opt.spell_check
            && !opt.free_typing
            && (!vs::tone_allowed_with(&lowered, t, opt.allow_coda_k)
                || vs::check_with(&lowered, t, opt.allow_coda_k) == Status::Invalid)
        {
            let _ = st.letters.try_push(Letter::literal(k));
            return;
        }
        st.tone = t;
        st.transformed = true;
        return;
    }
    if lk == im.remove_tone && has_vowel && (st.tone != 0 || st.transformed) {
        if st.tone != 0 {
            st.tone = 0;
        } else {
            for l in st.letters.iter_mut() {
                if matches!(l.mark, Some(Mark::Circumflex | Mark::Breve | Mark::Horn)) {
                    l.mark = None;
                }
            }
        }
        return;
    }

    if let Some(rules) = im.marks_for(lk) {
        let cands = candidates(st, rules);
        if !cands.is_empty() {
            let applied: Vec<Cand> =
                cands.iter().filter(|c| is_applied(st.letters.as_slice(), c.as_slice())).cloned().collect();
            let pending: Vec<Cand> = cands.iter().filter(|c| !applied.contains(*c)).cloned().collect();

            for c in &pending {
                let old: Vec<(usize, Option<Mark>, bool)> =
                    c.iter().map(|&(i, _)| (i, st.letters[i].mark, st.letters[i].auto_horn)).collect();
                let mut protected = false;
                for &(i, m) in c {
                    if c.len() == 2 && st.letters[i].base == 'u' && st.letters[i].mark != Some(m) {
                        let onset = vs::parse(&toneless_lower(&st.letters)).onset;
                        st.letters[i].auto_horn = OPEN_UO_ONSETS.contains(&onset.as_str());
                    }
                    st.letters[i].mark = Some(m);
                    if m == Mark::Stroke && !st.keep {
                        if i == 0 {
                            protected = st.letters[i + 1..].iter().all(|l| !vs::is_vowel(l.base));
                        } else if !vs::is_vowel(st.letters[i - 1].base) {
                            protected = true;
                        }
                        if protected {
                            st.keep = true;
                            st.keep_pos = i;
                        }
                    }
                }
                if protected {
                    st.transformed = true;
                    return;
                }
                if !opt.spell_check
                    || opt.free_typing
                    || status(&st.letters, st.tone, opt) != Status::Invalid
                {
                    st.transformed = true;
                    return;
                }
                for (i, m, a) in old {
                    st.letters[i].mark = m;
                    st.letters[i].auto_horn = a;
                }
            }

            if let Some(c) = applied.first() {
                let iu = c[0].0;
                if c.len() == 2 && st.letters[iu].auto_horn && c[1].0 == st.letters.len() - 1 {
                    st.letters[iu].auto_horn = false;
                    return;
                }
                st.undone = true;
                st.undo_key = Some(lk);
                if !st.locked.contains(&lk) {
                    st.locked.push(lk);
                }
                let i0 = c[0].0;
                if c.len() == 1 && st.letters[i0].from_standalone {
                    let up = st.letters[i0].upper;
                    st.letters[i0] = Letter::new(lk, up);
                    st.latched = true;
                    if i0 != st.letters.len() - 1 {
                        let _ = st.letters.try_push(Letter::literal(k));
                    }
                    return;
                }
                for &(i, _) in c {
                    st.letters[i].mark = None;
                }
                let _ = st.letters.try_push(Letter::literal(k));
                return;
            }

            if !pending.is_empty() && (!opt.spell_check || opt.free_typing) {
                for &(i, m) in &pending[0] {
                    st.letters[i].mark = Some(m);
                }
                st.transformed = true;
                return;
            }
        }
    }

    if opt.bracket_keys
        && let Some((base, mark)) = im.insert_for(k)
    {
        let _ = st.letters.try_push(Letter {
            base,
            upper: false,
            mark: Some(mark),
            from_standalone: true,
            auto_horn: false,
        });
        st.transformed = true;
        return;
    }
    if opt.w_as_u
        && let Some((base, mark)) = im.standalone_for(lk)
    {
        let letter =
            Letter { base, upper: k != lk, mark: Some(mark), from_standalone: true, auto_horn: false };
        let mut cand = st.letters.clone();
        let _ = cand.try_push(letter);
        if !opt.spell_check || opt.free_typing || status(&cand, st.tone, opt) != Status::Invalid {
            let _ = st.letters.try_push(letter);
            st.transformed = true;
            return;
        }
    }

    let _ = st.letters.try_push(Letter::literal(k));
}

fn result(st: &WordState, raw: &[char], opt: &TransformOptions, is_final: bool) -> TransformResult {
    let text = render(st, opt.tone_style);
    let stt = status_for(&st.letters, st.tone, opt, is_final);
    if opt.spell_check && opt.auto_restore_keys && st.transformed {
        let (start, end) = cluster_span(&st.letters);
        let raw_s: String = raw.iter().collect();
        let low = raw_s.to_lowercase();
        let deliberate_undo = st.undone
            && st.undo_key.is_some_and(|k| {
                let key: String = k.to_lowercase().collect();
                let double = format!("{key}{key}");
                let triple = format!("{key}{key}{key}");
                low.ends_with(&double) || low.contains(&triple)
            });
        let toneless = toneless_lower(&st.letters);
        let open_uo = start > 0 && (toneless.ends_with("ươ") || toneless.ends_with("uơ"));
        let bad_partial = is_final && stt == Status::Partial && end > start && start > 0 && !open_uo;
        let keep_ok = st.keep && st.letters[st.keep_pos + 1..].iter().all(|l| !vs::is_vowel(l.base));
        let bad = !keep_ok && !deliberate_undo && (stt == Status::Invalid || bad_partial);
        let rendered = if st.undone && st.latched { text.clone() } else { String::new() };
        if !rendered.is_empty() {
            return TransformResult { text: rendered.clone(), restored: false, status: stt, rendered };
        }
        if bad {
            return TransformResult { text: raw_s, restored: true, status: stt, rendered };
        }
        return TransformResult { text, restored: false, status: stt, rendered };
    }
    TransformResult { text, restored: false, status: stt, rendered: String::new() }
}

/// Biến đổi một lát cắt phím thô `raw` thành [`TransformResult`].
pub fn transform(
    raw: &[char],
    im: &InputMethodSpec,
    opt: &TransformOptions,
    is_final: bool,
) -> TransformResult {
    if raw.len() > MAX_RAW {
        return TransformResult {
            text: raw.iter().collect(),
            restored: false,
            status: Status::Invalid,
            rendered: String::new(),
        };
    }
    if let Some(lit) = vni_literal(raw, im) {
        return TransformResult {
            text: lit,
            restored: true,
            status: Status::Invalid,
            rendered: String::new(),
        };
    }
    let mut st = WordState::default();
    for &k in raw {
        step(&mut st, k, im, opt);
    }
    result(&st, raw, opt, is_final)
}

/// Biến đổi một chuỗi phím thô `&str` thành [`TransformResult`].
///
/// # Examples
///
/// ```
/// use vntype_core::{transform_str, TransformOptions, TELEX};
///
/// let res = transform_str("tieengs", &TELEX, &TransformOptions::default(), false);
/// assert_eq!(res.text, "tiếng");
/// ```
pub fn transform_str(
    raw: &str,
    im: &InputMethodSpec,
    opt: &TransformOptions,
    is_final: bool,
) -> TransformResult {
    let cs: Vec<char> = raw.chars().collect();
    transform(&cs, im, opt, is_final)
}

/// Máy trạng thái biến đổi tăng dần từng phím gõ.
///
/// Mỗi lần gọi [`Composer::push`], máy chỉ tính toán bước chuyển tiếp tiếp theo dựa trên
/// trạng thái đỉnh ngăn xếp, cho phép lùi lại nhanh chóng và đảm bảo kết quả trùng khớp
/// hoàn toàn với hàm thuần [`transform`].
///
/// # Examples
///
/// ```
/// use vntype_core::{Composer, TransformOptions, TELEX};
///
/// let mut c = Composer::new(&TELEX, TransformOptions::default());
/// for ch in "vieejt".chars() {
///     c.push(ch);
/// }
/// assert_eq!(c.result(true).text, "việt");
/// ```
#[derive(Debug, Clone)]
pub struct Composer {
    im: &'static InputMethodSpec,
    opt: TransformOptions,
    raw: Vec<char>,
    states: Vec<WordState>,
}

impl Composer {
    /// Khởi tạo một [`Composer`] rỗng với kiểu gõ và tuỳ chọn chỉ định.
    pub fn new(im: &'static InputMethodSpec, opt: TransformOptions) -> Self {
        Composer { im, opt, raw: Vec::with_capacity(MAX_RAW + 1), states: vec![WordState::default()] }
    }

    /// Khởi tạo một [`Composer`] và nạp sẵn danh sách phím thô `raw`.
    pub fn with_raw(im: &'static InputMethodSpec, opt: TransformOptions, raw: &[char]) -> Self {
        let mut c = Composer::new(im, opt);
        for &k in raw {
            c.push(k);
        }
        c
    }

    /// Đẩy thêm một phím thô vào máy trạng thái.
    pub fn push(&mut self, k: char) {
        self.raw.push(k);
        if self.raw.len() > MAX_RAW {
            return;
        }
        let mut st = self.states.last().cloned().unwrap_or_default();
        step(&mut st, k, self.im, &self.opt);
        self.states.push(st);
    }

    /// Bỏ phím thô cuối cùng khỏi máy trạng thái.
    pub fn pop(&mut self) {
        if !self.raw.is_empty() {
            if self.raw.len() <= MAX_RAW {
                self.states.pop();
            }
            self.raw.pop();
        }
    }

    /// Trả về lát cắt các phím thô đang lưu trong máy trạng thái.
    pub fn raw(&self) -> &[char] {
        &self.raw
    }

    /// Xuất kết quả biến đổi tại trạng thái hiện tại.
    pub fn result(&self, is_final: bool) -> TransformResult {
        if self.raw.len() > MAX_RAW {
            return TransformResult {
                text: self.raw.iter().collect(),
                restored: false,
                status: Status::Invalid,
                rendered: String::new(),
            };
        }
        if let Some(lit) = vni_literal(&self.raw, self.im) {
            return TransformResult {
                text: lit,
                restored: true,
                status: Status::Invalid,
                rendered: String::new(),
            };
        }
        let st = self.states.last().expect("ngăn xếp trạng thái luôn có ít nhất 1 phần tử");
        result(st, &self.raw, &self.opt, is_final)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{SIMPLE_TELEX, TELEX, VIQR, VNI};

    #[test]
    fn telex_basic_and_free_tone() {
        let opt = TransformOptions::default();
        for (raw, expected) in [
            ("tieengs", "tiếng"),
            ("tiseeng", "tiếng"),
            ("thuowng", "thương"),
            ("muowif", "mười"),
            ("huow", "huơ"),
            ("giuf", "giù"),
            ("quas", "quá"),
            ("Vieejt", "Việt"),
            ("Dda", "Đa"),
        ] {
            assert_eq!(transform_str(raw, &TELEX, &opt, false).text, expected, "raw={raw}");
        }
    }

    #[test]
    fn other_input_methods() {
        let opt = TransformOptions::default();
        assert_eq!(transform_str("tie6ng1", &VNI, &opt, false).text, "tiếng");
        assert_eq!(transform_str("d9a2", &VNI, &opt, false).text, "đà");
        assert_eq!(transform_str("a11", &VNI, &opt, false).text, "a1");
        assert_eq!(transform_str("test12", &VNI, &opt, false).text, "test12");

        assert_eq!(transform_str("w", &SIMPLE_TELEX, &opt, false).text, "w");
        assert_eq!(transform_str("thuowng", &SIMPLE_TELEX, &opt, false).text, "thương");

        assert_eq!(transform_str("tie^'ng", &VIQR, &opt, false).text, "tiếng");
        assert_eq!(transform_str("dda`", &VIQR, &opt, false).text, "đà");
    }

    #[test]
    fn undo_latch_and_standalone() {
        let opt = TransformOptions::default();
        let r_tone = transform_str("tieengss", &TELEX, &opt, false);
        assert_eq!(r_tone.text, "tiêngs");
        assert!(!r_tone.rendered.is_empty());

        assert_eq!(transform_str("aaa", &TELEX, &opt, false).text, "aa");
        assert_eq!(transform_str("w", &TELEX, &opt, false).text, "ư");
        assert_eq!(transform_str("ww", &TELEX, &opt, false).text, "w");
    }

    #[test]
    fn transform_options_behavior() {
        let mut opt = TransformOptions { w_as_u: false, ..Default::default() };
        assert_eq!(transform_str("w", &TELEX, &opt, false).text, "w");

        opt = TransformOptions { bracket_keys: true, ..Default::default() };
        assert_eq!(transform_str("[", &TELEX, &opt, false).text, "ơ");
        assert_eq!(transform_str("]", &TELEX, &opt, false).text, "ư");

        opt = TransformOptions { quick_telex: true, ..Default::default() };
        assert_eq!(transform_str("ccon", &TELEX, &opt, false).text, "chon");

        opt = TransformOptions { allow_coda_k: false, ..Default::default() };
        assert!(transform_str("Ddawks", &TELEX, &opt, true).restored);
        opt.allow_coda_k = true;
        assert_eq!(transform_str("Ddawks", &TELEX, &opt, true).text, "Đắk");

        opt = TransformOptions { tone_style: ToneStyle::New, ..Default::default() };
        assert_eq!(transform_str("hoaf", &TELEX, &opt, false).text, "hoà");

        let eng = transform_str("water", &TELEX, &TransformOptions::default(), true);
        assert!(eng.restored && eng.text == "water");
        assert_eq!(transform_str("ddc", &TELEX, &TransformOptions::default(), true).text, "đc");
    }

    #[test]
    fn incremental_composer_push_pop_and_max_raw() {
        let opt = TransformOptions::default();
        let mut c = Composer::new(&TELEX, opt);
        let keys: Vec<char> = "tieengs".chars().collect();
        for (i, &k) in keys.iter().enumerate() {
            c.push(k);
            assert_eq!(c.result(false), transform(&keys[..=i], &TELEX, &opt, false));
        }
        assert_eq!(c.raw(), keys.as_slice());
        c.pop();
        assert_eq!(c.result(false).text, "tiêng");

        let long = "a".repeat(MAX_RAW + 1);
        assert_eq!(transform_str(&long, &TELEX, &opt, false).text, long);
        let mut c_long = Composer::with_raw(&TELEX, opt, &long.chars().collect::<Vec<_>>());
        assert_eq!(c_long.result(false).text, long);
        c_long.pop();
        assert_eq!(c_long.raw().len(), MAX_RAW);
    }
}
