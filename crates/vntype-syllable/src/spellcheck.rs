//! Bộ kiểm tra chính tả tiếng Việt ba trạng thái (`Valid`, `Partial`, `Invalid`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::iter;
use spin::Once;

use crate::phonology::{is_letter, is_vowel, parse, FRONT_VOWELS, ONSETS};

/// Trạng thái hợp lệ chính tả của một âm tiết đang gõ hoặc đã hoàn chỉnh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// Âm tiết tiếng Việt hoàn chỉnh và đúng quy tắc chính tả.
    Valid,
    /// Âm tiết đang gõ dở nhưng có thể trở thành hợp lệ khi thêm chữ hoặc dấu phụ.
    Partial,
    /// Chuỗi ký tự vi phạm quy tắc kết hợp âm vị tiếng Việt.
    Invalid,
}

const CODAS: &[&str] = &["c", "ch", "m", "n", "ng", "nh", "p", "t"];
const STOP_CODAS: &[&str] = &["c", "ch", "p", "t"];
const CODAS_WITH_K: &[&str] = &["c", "ch", "m", "n", "ng", "nh", "p", "t", "k"];
const STOP_CODAS_WITH_K: &[&str] = &["c", "ch", "p", "t", "k"];
const CH_NH_CLUSTERS: &[&str] = &["a", "ê", "i", "oa", "uê", "uy"];

const OPEN_CLUSTERS: &[&str] = &[
    "a", "e", "ê", "i", "o", "ô", "ơ", "u", "ư", "y", "ai", "ao", "au", "ay", "âu", "ây", "eo", "êu", "ia",
    "iu", "oa", "oe", "oi", "ôi", "ơi", "ua", "uê", "ui", "uy", "ưa", "ưi", "ưu", "uơ", "iêu", "yêu",
    "oai", "oay", "oao", "oeo", "uây", "uôi", "ươi", "ươu", "uya", "uyu",
];

const CLOSED_CLUSTERS: &[&str] = &[
    "a", "ă", "â", "e", "ê", "i", "o", "ô", "ơ", "u", "ư", "oa", "oă", "oe", "uâ", "uê", "uy", "iê", "yê",
    "uô", "ươ", "uyê", "oo",
];

struct Tables {
    onset_prefixes: BTreeSet<String>,
    open: BTreeSet<String>,
    closed: BTreeSet<String>,
    ch_nh: BTreeSet<String>,
    relaxed_any: BTreeSet<String>,
    relaxed_for_coda: BTreeMap<String, BTreeSet<String>>,
    relaxed_for_coda_k: BTreeMap<String, BTreeSet<String>>,
}

fn prefixes(s: &str) -> Vec<String> {
    let cs: Vec<char> = s.chars().collect();
    (1..=cs.len()).map(|i| cs[..i].iter().collect()).collect()
}

fn demark(c: char) -> char {
    match c {
        'ă' | 'â' => 'a',
        'ê' => 'e',
        'ô' | 'ơ' => 'o',
        'ư' => 'u',
        _ => c,
    }
}

fn relax(s: &str) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = iter::once(String::new()).collect();
    for ch in s.chars() {
        let opts = [ch, demark(ch)];
        let mut next = BTreeSet::new();
        for p in &out {
            for o in opts {
                let mut q = p.clone();
                q.push(o);
                next.insert(q);
            }
        }
        out = next;
    }
    out
}

fn tables() -> &'static Tables {
    static T: Once<Tables> = Once::new();
    T.call_once(|| {
        let mut onset_prefixes = BTreeSet::new();
        for o in ONSETS.iter().copied().chain(["gi", "qu"]) {
            onset_prefixes.extend(prefixes(o));
        }
        let open: BTreeSet<String> = OPEN_CLUSTERS.iter().map(|s| s.to_string()).collect();
        let closed: BTreeSet<String> = CLOSED_CLUSTERS.iter().map(|s| s.to_string()).collect();
        let ch_nh: BTreeSet<String> = CH_NH_CLUSTERS.iter().map(|s| s.to_string()).collect();

        let mut relaxed_any = BTreeSet::new();
        for c in open.iter().chain(closed.iter()) {
            for p in prefixes(c) {
                relaxed_any.extend(relax(&p));
            }
        }

        let mut relaxed_for_coda: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for coda in CODAS {
            for cl in CLOSED_CLUSTERS {
                if (*coda == "ch" || *coda == "nh") && !ch_nh.contains(*cl) {
                    continue;
                }
                for p in prefixes(coda) {
                    relaxed_for_coda.entry(p).or_default().extend(relax(cl));
                }
            }
        }
        let mut relaxed_for_coda_k = relaxed_for_coda.clone();
        for cl in CLOSED_CLUSTERS {
            relaxed_for_coda_k.entry("k".to_string()).or_default().extend(relax(cl));
        }

        Tables { onset_prefixes, open, closed, ch_nh, relaxed_any, relaxed_for_coda, relaxed_for_coda_k }
    })
}

fn onset_ok(onset: &str, first: char) -> bool {
    let front = FRONT_VOWELS.contains(first);
    if matches!(onset, "k" | "gh" | "ngh") && !front {
        return false;
    }
    if matches!(onset, "c" | "ng" | "q") && front {
        return false;
    }
    if onset == "g" && (first == 'e' || first == 'ê') {
        return false;
    }
    if onset == "qu" && (first == 'u' || first == 'ư') {
        return false;
    }
    true
}

/// Kiểm tra chính tả khi đang gõ của một chuỗi chữ thường không dấu thanh (không cho phép âm cuối `k`).
pub fn check(s: &str, tone: u8) -> Status {
    check_with(s, tone, false)
}

/// Kiểm tra chính tả khi đang gõ của một chuỗi chữ thường không dấu thanh, với tùy chọn cho phép âm cuối `k`.
pub fn check_with(s: &str, tone: u8, allow_coda_k: bool) -> Status {
    if s.is_empty() {
        return Status::Partial;
    }
    if s.chars().any(|c| !is_letter(c)) {
        return Status::Invalid;
    }
    let t = tables();
    let syl = parse(s);
    if syl.cluster.is_empty() {
        if !syl.coda.is_empty() {
            return if t.onset_prefixes.contains(s) { Status::Partial } else { Status::Invalid };
        }
        return Status::Partial;
    }
    if syl.coda.chars().any(is_vowel) {
        return Status::Invalid;
    }
    let mut cluster = syl.cluster.clone();
    if syl.onset == "qu" && cluster.starts_with('y') {
        cluster.insert(0, 'u');
    }
    let first = syl.cluster.chars().next().unwrap_or('a');
    if !onset_ok(&syl.onset, first) {
        return Status::Invalid;
    }
    let stop_bad = matches!(tone, 2..=4);
    let codas = if allow_coda_k { CODAS_WITH_K } else { CODAS };
    let stop_codas = if allow_coda_k { STOP_CODAS_WITH_K } else { STOP_CODAS };
    let relaxed_for_coda = if allow_coda_k { &t.relaxed_for_coda_k } else { &t.relaxed_for_coda };
    let coda = syl.coda.as_str();

    if coda.is_empty() {
        if t.open.contains(&cluster) {
            return Status::Valid;
        }
    } else if codas.contains(&coda)
        && t.closed.contains(&cluster)
        && !((coda == "ch" || coda == "nh") && !t.ch_nh.contains(&cluster))
    {
        if stop_codas.contains(&coda) && stop_bad {
            return Status::Invalid;
        }
        return Status::Valid;
    }

    if coda.is_empty() {
        return if t.relaxed_any.contains(&cluster) { Status::Partial } else { Status::Invalid };
    }
    if let Some(set) = relaxed_for_coda.get(coda) {
        if set.contains(&cluster) {
            if stop_bad && stop_codas.contains(&coda) {
                return Status::Invalid;
            }
            return Status::Partial;
        }
    }
    Status::Invalid
}

/// Kiểm tra chính tả khi từ đã kết thúc (âm cuối tắc `c/ch/p/t/k` bắt buộc mang thanh sắc `1` hoặc nặng `5`).
pub fn check_final(s: &str, tone: u8, allow_coda_k: bool) -> Status {
    let st = check_with(s, tone, allow_coda_k);
    if st != Status::Valid {
        return st;
    }
    let syl = parse(s);
    let stop_codas = if allow_coda_k { STOP_CODAS_WITH_K } else { STOP_CODAS };
    if stop_codas.contains(&syl.coda.as_str()) && !matches!(tone, 1 | 5) {
        Status::Invalid
    } else {
        st
    }
}

/// Kiểm tra mã thanh có hợp lệ với âm cuối khi đang gõ hay không (âm cuối tắc không đi với thanh huyền, hỏi, ngã).
pub fn tone_allowed(s: &str, tone: u8) -> bool {
    tone_allowed_with(s, tone, false)
}

/// Kiểm tra mã thanh có hợp lệ với âm cuối khi đang gõ hay không, với tùy chọn cho phép âm cuối `k`.
pub fn tone_allowed_with(s: &str, tone: u8, allow_coda_k: bool) -> bool {
    let syl = parse(s);
    let stop_codas = if allow_coda_k { STOP_CODAS_WITH_K } else { STOP_CODAS };
    !(stop_codas.contains(&syl.coda.as_str()) && matches!(tone, 2..=4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_final_stop_coda_requires_tone() {
        assert_eq!(check_final("dat", 0, false), Status::Invalid);
        assert_eq!(check_final("dat", 1, false), Status::Valid);
        assert_eq!(check_final("mô", 0, false), Status::Valid);
        assert_eq!(check_final("đăk", 1, true), Status::Valid);
        assert_eq!(check_final("đăk", 0, true), Status::Invalid);
    }
}
