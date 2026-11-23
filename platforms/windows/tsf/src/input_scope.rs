//! Ánh xạ thuộc tính `ITfInputScope` của Windows TSF sang [`vntype_engine::FieldKind`].

use vntype_engine::FieldKind;

/// Mã hằng số `InputScope` chuẩn từ `<inputscope.h>` của Windows SDK.
pub mod win_input_scope {
    /// Ô văn bản mặc định (`IS_DEFAULT`).
    pub const IS_DEFAULT: i32 = 0;
    /// Ô địa chỉ URL (`IS_URL`).
    pub const IS_URL: i32 = 1;
    /// Ô địa chỉ email (`IS_EMAIL_SMTPEMAILADDRESS`).
    pub const IS_EMAIL_SMTPEMAILADDRESS: i32 = 5;
    /// Ô nhập số (`IS_NUMBER`).
    pub const IS_NUMBER: i32 = 29;
    /// Ô mật khẩu (`IS_PASSWORD`).
    pub const IS_PASSWORD: i32 = 31;
    /// Ô số điện thoại (`IS_TELEPHONE_FULLTELEPHONENUMBER`).
    pub const IS_TELEPHONE_FULLTELEPHONENUMBER: i32 = 32;
    /// Ô chữ số (`IS_DIGITS`).
    pub const IS_DIGITS: i32 = 28;
    /// Ô mã PIN số (`IS_NUMERIC_PIN`).
    pub const IS_NUMERIC_PIN: i32 = 64;
    /// Ô mã PIN chữ và số (`IS_ALPHANUMERIC_PIN`).
    pub const IS_ALPHANUMERIC_PIN: i32 = 65;
}

/// Xác định [`FieldKind`] từ danh sách các mã `InputScope` của vùng chọn hiện tại và tên tiến trình.
pub fn resolve_field_kind_from_scopes(scopes: &[i32], app_name: &str) -> FieldKind {
    use win_input_scope::*;
    let mut result = FieldKind::Text;
    for &scope in scopes {
        match scope {
            IS_PASSWORD | IS_NUMERIC_PIN | IS_ALPHANUMERIC_PIN => {
                return FieldKind::Password;
            }
            IS_URL => {
                result = FieldKind::Url;
            }
            IS_EMAIL_SMTPEMAILADDRESS if result == FieldKind::Text || result == FieldKind::Url => {
                result = FieldKind::Email;
            }
            IS_NUMBER | IS_DIGITS | IS_TELEPHONE_FULLTELEPHONENUMBER if result == FieldKind::Text => {
                result = FieldKind::Number;
            }
            _ => {}
        }
    }
    if result == FieldKind::Text && is_windows_terminal_app(app_name) {
        return FieldKind::Terminal;
    }
    result
}

/// Kiểm tra một tên tiến trình trên Windows có phải là trình giả lập dòng lệnh Console/Terminal hay không.
pub fn is_windows_terminal_app(app: &str) -> bool {
    matches!(
        app.trim().to_lowercase().as_str(),
        "windowsterminal" | "openconsole" | "cmd" | "conhost" | "powershell" | "pwsh" | "wt"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use win_input_scope::*;

    #[test]
    fn input_scope_priority_and_terminal_detection() {
        assert_eq!(resolve_field_kind_from_scopes(&[IS_URL, IS_PASSWORD], "chrome"), FieldKind::Password);
        assert_eq!(resolve_field_kind_from_scopes(&[IS_URL], "chrome"), FieldKind::Url);
        assert_eq!(resolve_field_kind_from_scopes(&[], "WindowsTerminal"), FieldKind::Terminal);
        assert_eq!(resolve_field_kind_from_scopes(&[], "notepad"), FieldKind::Text);
    }
}
