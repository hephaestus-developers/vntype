//! Chính sách xử lý theo từng loại trường nhập liệu (`FieldKind`).

use vntype_engine::FieldKind;

/// Kiểm tra xem trường nhập liệu có bị tắt hoàn toàn bộ gõ tiếng Việt hay không (Mật khẩu, Số, Email).
///
/// Riêng trường `FieldKind::Url` vẫn cho phép gõ tiếng Việt để tìm kiếm trên thanh địa chỉ trình duyệt,
/// nhưng engine sẽ tự động bật chế độ an toàn URL.
pub fn is_disabled(kind: FieldKind) -> bool {
    matches!(kind, FieldKind::Password | FieldKind::Number | FieldKind::Email)
}

/// Kiểm tra xem trường nhập liệu có phải là giao diện dòng lệnh Terminal hay không.
pub fn is_terminal(kind: FieldKind) -> bool {
    matches!(kind, FieldKind::Terminal)
}

/// Kiểm tra xem trường nhập liệu có phải là ô mật khẩu nhạy cảm hay không.
///
/// Khi chuyển vào trường nhạy cảm, mọi vùng văn bản đang soạn dở phải bị huỷ bỏ ngay lập tức,
/// tuyệt đối không được chốt (commit) vào ô mật khẩu.
pub fn is_sensitive(kind: FieldKind) -> bool {
    matches!(kind, FieldKind::Password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_policy_checks() {
        assert!(is_disabled(FieldKind::Password));
        assert!(is_disabled(FieldKind::Number));
        assert!(is_disabled(FieldKind::Email));
        assert!(!is_disabled(FieldKind::Url));
        assert!(!is_disabled(FieldKind::Text));
        assert!(is_sensitive(FieldKind::Password));
    }
}
