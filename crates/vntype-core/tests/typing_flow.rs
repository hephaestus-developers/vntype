//! Integration tests kiểm tra luồng gõ phím, hiệu chuỗi, khôi phục và chuyển bảng mã của `vntype-core`.

use vntype_core::{
    Composer, Edit, InputMethodSpec, TELEX, TransformOptions, VNI, convert_charset, diff, rebuild,
    remove_diacritics,
};

#[test]
fn end_to_end_typing_diff_and_rebuild() {
    let opt = TransformOptions::default();
    let im = InputMethodSpec::by_name("telex").expect("phải tìm thấy kiểu gõ telex");
    let mut composer = Composer::new(im, opt);

    let mut screen = String::new();
    for ch in "tieengs".chars() {
        composer.push(ch);
        let next = composer.result(false).text;
        let edit = diff(&screen, &next);
        let keep_chars = screen.chars().count() - edit.delete_before;
        screen = screen.chars().take(keep_chars).collect::<String>() + &edit.insert;
    }
    assert_eq!(screen, "tiếng");
    assert_eq!(diff("tiêng", "tiếng"), Edit { delete_before: 3, insert: "ếng".into() });

    let raw = rebuild(&screen, &TELEX, &opt).expect("phải khôi phục được phím thô");
    let mut restored = Composer::with_raw(&TELEX, opt, &raw);
    restored.pop();
    assert_eq!(restored.result(false).text, "tiêng");
}

#[test]
fn end_to_end_charset_conversion_after_typing() {
    let opt = TransformOptions::default();
    let mut composer = Composer::new(&VNI, opt);
    for ch in "Vie6t5".chars() {
        composer.push(ch);
    }
    let typed = composer.result(true).text;
    assert_eq!(typed, "Việt");

    let viqr = convert_charset(&typed, "unicode", "viqr");
    assert_eq!(viqr, "Vie^.t");
    assert_eq!(convert_charset(&viqr, "viqr", "unicode"), "Việt");
    assert_eq!(remove_diacritics(&typed), "Viet");
}
