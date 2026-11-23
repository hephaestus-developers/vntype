//! Quản lý định danh GUID (`CLSID`, `GUID_Profile`, `GUID_DisplayAttribute`) và đăng ký Windows TSF.

/// Định danh lớp COM (`CLSID`) của VNType Text Service.
pub const CLSID_VNTYPE_TEXT_SERVICE: u128 = 0x8d4a2b71_5e19_4c3a_9f21_7b6e5d4c3a21;

/// Định danh hồ sơ ngôn ngữ (`GUID_Profile`) tiếng Việt (`vi-VN`, `LANGID = 0x042a`).
pub const GUID_VNTYPE_PROFILE: u128 = 0x9e5b3c82_6f2a_4d4b_8a32_8c7f6e5d4b32;

/// Định danh thuộc tính hiển thị gạch chân (`GUID_DisplayAttribute`) cho vùng Preedit.
pub const GUID_VNTYPE_DISPLAY_ATTRIBUTE: u128 = 0xaf6c4d93_703b_4e5c_9b43_9d807f6e5c43;

/// Mã ngôn ngữ Windows (`LANGID`) cho tiếng Việt (`vi-VN`).
pub const LANGID_VIETNAMESE: u16 = 0x042a;

/// Thông tin cấu hình đăng ký COM Server trong Windows Registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsfRegistrationSpec {
    /// Đường dẫn tuyệt đối tới tệp `vntype_tsf.dll`.
    pub dll_path: String,
    /// Tên hiển thị trong thanh ngôn ngữ Windows.
    pub display_name: String,
    /// Đăng ký ở nhánh người dùng hiện tại (`HKCU`) hay toàn máy (`HKLM`).
    pub per_user: bool,
}

impl TsfRegistrationSpec {
    /// Tạo đặc tả đăng ký mặc định cho người dùng hiện tại (`HKCU`).
    pub fn per_user(dll_path: impl Into<String>) -> Self {
        TsfRegistrationSpec {
            dll_path: dll_path.into(),
            display_name: "VNType (Tiếng Việt)".into(),
            per_user: true,
        }
    }

    /// Trả về khoá Registry gốc của lớp COM `InprocServer32`.
    pub fn clsid_registry_key(&self) -> String {
        let root = if self.per_user { "HKCU\\Software\\Classes" } else { "HKLM\\SOFTWARE\\Classes" };
        format!("{root}\\CLSID\\{{8D4A2B71-5E19-4C3A-9F21-7B6E5D4C3A21}}\\InprocServer32")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_clsid_path() {
        let spec = TsfRegistrationSpec::per_user("C:\\VNType\\vntype_tsf.dll");
        assert!(spec.clsid_registry_key().starts_with("HKCU\\Software\\Classes\\CLSID\\"));
    }
}
