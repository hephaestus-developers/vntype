//! Bảng dữ liệu ánh xạ tĩnh cho 14 bảng mã tiếng Việt truyền thống (được sinh lúc biên dịch từ `data/charsets/`).

/// Định nghĩa một bảng mã tĩnh gồm khoá định danh, tên hiển thị và danh sách cặp `(ký_tự_unicode, mã_đích)`.
#[derive(Debug, Clone, Copy)]
pub struct StaticCharsetSpec {
    /// Định danh chuẩn của bảng mã (ví dụ: `"tcvn3"`, `"vni_windows"`).
    pub key: &'static str,
    /// Tên hiển thị đầy đủ của bảng mã.
    pub display_name: &'static str,
    /// Mảng tĩnh ánh xạ từ ký tự Unicode NFC sang chuỗi mã của bảng mã đích.
    pub mappings: &'static [(char, &'static str)],
}

include!(concat!(env!("OUT_DIR"), "/charset_tables_gen.rs"));
