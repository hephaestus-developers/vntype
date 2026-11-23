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

#[cfg(windows)]
pub mod win_input_scope_query {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::TextServices::{
        GUID_PROP_INPUTSCOPE, ITfContext, ITfInputScope, ITfRange, TF_SELECTION, TF_SELECTIONSTYLE,
    };
    use windows_core::Interface;

    /// Truy vấn danh sách mã `InputScope` từ `ITfContext` hiện tại qua `GUID_PROP_INPUTSCOPE`.
    pub fn query_context_input_scopes(context: &ITfContext, ec: u32) -> Vec<i32> {
        unsafe {
            let mut sel = [TF_SELECTION {
                range: core::mem::ManuallyDrop::new(None),
                style: TF_SELECTIONSTYLE::default(),
            }];
            let mut fetched = 0u32;
            if context.GetSelection(ec, 0, &mut sel, &mut fetched).is_err() || fetched == 0 {
                return Vec::new();
            }
            let range_opt: Option<ITfRange> =
                core::mem::ManuallyDrop::into_inner(core::ptr::read(&sel[0].range));
            let Some(range) = range_opt else {
                return Vec::new();
            };

            let Ok(prop) = context.GetProperty(&GUID_PROP_INPUTSCOPE) else {
                return Vec::new();
            };
            let mut var = VARIANT::default();
            if prop.GetValue(ec, &range, &mut var).is_err() {
                return Vec::new();
            }

            let Ok(unk) = windows_core::IUnknown::try_from(&var) else {
                return Vec::new();
            };
            let Ok(input_scope) = unk.cast::<ITfInputScope>() else {
                return Vec::new();
            };

            let mut ptr = core::ptr::null_mut();
            let mut count = 0u32;
            if input_scope.GetInputScopes(&mut ptr, &mut count).is_err() || ptr.is_null() {
                return Vec::new();
            }
            let slice = core::slice::from_raw_parts(ptr, count as usize);
            let scopes: Vec<i32> = slice.iter().map(|s| s.0).collect();
            CoTaskMemFree(Some(ptr as *const core::ffi::c_void));
            scopes
        }
    }
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
