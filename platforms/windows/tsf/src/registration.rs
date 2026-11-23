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

#[cfg(windows)]
pub mod win_reg {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW,
        RegDeleteTreeW, RegSetValueExW,
    };
    use windows::Win32::UI::TextServices::{
        CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
        GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_COMLESS, GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
        GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr, ITfInputProcessorProfileMgr,
    };
    use windows_core::{GUID, PCWSTR};

    use super::{CLSID_VNTYPE_TEXT_SERVICE, GUID_VNTYPE_PROFILE, LANGID_VIETNAMESE, TsfRegistrationSpec};

    const CLSID_GUID: GUID = GUID::from_u128(CLSID_VNTYPE_TEXT_SERVICE);
    const PROFILE_GUID: GUID = GUID::from_u128(GUID_VNTYPE_PROFILE);

    fn to_wide(s: &str) -> Vec<u16> {
        let mut w: Vec<u16> = s.encode_utf16().collect();
        w.push(0);
        w
    }

    fn write_reg_sz(subkey: &str, value_name: Option<&str>, data: &str) -> windows_core::Result<()> {
        let sub_w = to_wide(subkey);
        let data_w = to_wide(data);
        let name_w = value_name.map(to_wide);
        let mut hkey = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(sub_w.as_ptr()),
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            )
            .ok()?;
            let name_ptr = name_w.as_ref().map(|v| PCWSTR(v.as_ptr())).unwrap_or(PCWSTR::null());
            let bytes = core::slice::from_raw_parts(
                data_w.as_ptr() as *const u8,
                data_w.len() * core::mem::size_of::<u16>(),
            );
            let res = RegSetValueExW(hkey, name_ptr, Some(0), REG_SZ, Some(bytes)).ok();
            let _ = RegCloseKey(hkey);
            res
        }
    }

    /// Đăng ký COM InprocServer32 và hồ sơ TSF tiếng Việt (`vi-VN`) cho người dùng hiện tại.
    pub fn register_tsf_server(hmodule: HMODULE) -> windows_core::Result<()> {
        let mut buf = [0u16; 1024];
        let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut buf) };
        let dll_path = String::from_utf16_lossy(&buf[..len as usize]);
        let spec = TsfRegistrationSpec::per_user(&dll_path);

        let clsid_key = "Software\\Classes\\CLSID\\{8D4A2B71-5E19-4C3A-9F21-7B6E5D4C3A21}";
        let inproc_key = "Software\\Classes\\CLSID\\{8D4A2B71-5E19-4C3A-9F21-7B6E5D4C3A21}\\InprocServer32";
        write_reg_sz(clsid_key, None, &spec.display_name)?;
        write_reg_sz(inproc_key, None, &spec.dll_path)?;
        write_reg_sz(inproc_key, Some("ThreadingModel"), "Apartment")?;

        unsafe {
            let co_ok = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            let result = (|| -> windows_core::Result<()> {
                let prof_mgr: ITfInputProcessorProfileMgr =
                    CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
                let desc = to_wide(&spec.display_name);
                let icon_file = to_wide(&spec.dll_path);
                prof_mgr.RegisterProfile(
                    &CLSID_GUID,
                    LANGID_VIETNAMESE,
                    &PROFILE_GUID,
                    &desc[..desc.len() - 1],
                    &icon_file[..icon_file.len() - 1],
                    0,
                    None,
                    0,
                    true,
                    0,
                )?;

                let cat_mgr: ITfCategoryMgr =
                    CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
                for cat in [
                    GUID_TFCAT_TIP_KEYBOARD,
                    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
                    GUID_TFCAT_TIPCAP_COMLESS,
                    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
                ] {
                    cat_mgr.RegisterCategory(&CLSID_GUID, &cat, &CLSID_GUID)?;
                }
                Ok(())
            })();
            if co_ok {
                CoUninitialize();
            }
            result
        }
    }

    /// Gỡ đăng ký hồ sơ TSF và khoá COM khỏi Registry người dùng hiện tại.
    pub fn unregister_tsf_server() -> windows_core::Result<()> {
        unsafe {
            let co_ok = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            if let Ok(cat_mgr) =
                CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
            {
                for cat in [
                    GUID_TFCAT_TIP_KEYBOARD,
                    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
                    GUID_TFCAT_TIPCAP_COMLESS,
                    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
                ] {
                    let _ = cat_mgr.UnregisterCategory(&CLSID_GUID, &cat, &CLSID_GUID);
                }
            }
            if let Ok(prof_mgr) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
                &CLSID_TF_InputProcessorProfiles,
                None,
                CLSCTX_INPROC_SERVER,
            ) {
                let _ = prof_mgr.UnregisterProfile(&CLSID_GUID, LANGID_VIETNAMESE, &PROFILE_GUID, 0);
            }
            if co_ok {
                CoUninitialize();
            }

            let clsid_key = to_wide("Software\\Classes\\CLSID\\{8D4A2B71-5E19-4C3A-9F21-7B6E5D4C3A21}");
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(clsid_key.as_ptr()));
        }
        Ok(())
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
