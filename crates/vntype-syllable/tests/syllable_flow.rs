//! Integration tests kiểm tra luồng xử lý âm tiết đầu-cuối của `vntype-syllable`.

use vntype_syllable::{
    Mark, Status, ToneStyle, check, check_final, check_with, compose_char, decompose_char, lower, parse,
    place_tone, strip_tone, upper,
};

#[test]
fn end_to_end_syllable_pipeline() {
    let word = "Thương";
    let (toneless, tone) = strip_tone(word);
    assert_eq!(toneless, "Thương");
    assert_eq!(tone, 0);

    let lower_toneless: String = toneless.chars().map(lower).collect();
    assert_eq!(check(&lower_toneless, 2), Status::Valid);
    assert_eq!(check_final(&lower_toneless, 2, false), Status::Valid);

    let syl = parse(&lower_toneless);
    assert_eq!(syl.onset, "th");
    assert_eq!(syl.cluster, "ươ");
    assert_eq!(syl.coda, "ng");

    let with_tone = place_tone(&toneless, 2, ToneStyle::Old);
    assert_eq!(with_tone, "Thường");

    let (stripped, extracted_tone) = strip_tone(&with_tone);
    assert_eq!(stripped, "Thương");
    assert_eq!(extracted_tone, 2);
}

#[test]
fn stop_coda_and_case_preservation() {
    assert_eq!(check("đăk", 0), Status::Invalid);
    assert_eq!(check_with("đăk", 0, true), Status::Valid);
    assert_eq!(check_final("đăk", 0, true), Status::Invalid);
    assert_eq!(check_final("đăk", 1, true), Status::Valid);

    let ch = compose_char('a', Some(Mark::Breve), 1, true);
    assert_eq!(ch, 'Ắ');
    let d = decompose_char(ch);
    assert_eq!(d.base, 'a');
    assert_eq!(d.mark, Some(Mark::Breve));
    assert_eq!(d.tone, 1);
    assert!(d.upper);
    assert_eq!(upper('ắ'), 'Ắ');
}
