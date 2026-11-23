//! Thư viện In-Process COM DLL (`vntype_tsf.dll`) của VNType trên Windows.

pub mod composition;
pub mod edit_session;
pub mod input_scope;
pub mod registration;
pub mod text_service;

use std::sync::atomic::{AtomicU32, Ordering};

pub use composition::CompositionTracker;
pub use edit_session::{TsfContextBackend, apply_effects_in_edit_session};
pub use input_scope::{is_windows_terminal_app, resolve_field_kind_from_scopes};
pub use registration::{
    CLSID_VNTYPE_TEXT_SERVICE, GUID_VNTYPE_DISPLAY_ATTRIBUTE, GUID_VNTYPE_PROFILE, LANGID_VIETNAMESE,
    TsfRegistrationSpec,
};
pub use text_service::VntypeTextService;

/// Bộ đếm số lượng đối tượng COM đang hoạt động trong DLL để phục vụ `DllCanUnloadNow`.
static DLL_REF_COUNT: AtomicU32 = AtomicU32::new(0);

/// Tăng bộ đếm tham chiếu toàn cục của DLL khi tạo một đối tượng COM mới.
pub fn increment_dll_ref() -> u32 {
    DLL_REF_COUNT.fetch_add(1, Ordering::SeqCst) + 1
}

/// Giảm bộ đếm tham chiếu toàn cục của DLL khi huỷ một đối tượng COM.
pub fn decrement_dll_ref() -> u32 {
    DLL_REF_COUNT.fetch_sub(1, Ordering::SeqCst).saturating_sub(1)
}

/// Điểm xuất chuẩn COM `DllGetClassObject`: trả về `CLASS_E_CLASSNOTAVAILABLE` (`0x80040111`)
/// cho tới khi COM class factory Win32 được gắn hoàn chỉnh.
///
/// # Safety
///
/// Nếu `ppv` khác `NULL`, con trỏ phải trỏ tới vùng nhớ hợp lệ và căn chỉnh đúng để ghi con trỏ COM.
#[cfg(not(windows))]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    _rclsid: *const u8,
    _riid: *const u8,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if !ppv.is_null() {
        unsafe { *ppv = core::ptr::null_mut() };
    }
    0x8004_0111u32 as i32
}

/// Điểm xuất chuẩn COM `DllCanUnloadNow`: trả về `0` (`S_OK`) khi không còn đối tượng nào đang mở.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> i32 {
    if DLL_REF_COUNT.load(Ordering::SeqCst) == 0 {
        0 // S_OK
    } else {
        1 // S_FALSE
    }
}

/// Điểm xuất chuẩn COM `DllRegisterServer`: đăng ký Text Service cho người dùng hiện tại.
#[cfg(not(windows))]
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> i32 {
    0 // S_OK
}

/// Điểm xuất chuẩn COM `DllUnregisterServer`: gỡ đăng ký Text Service.
#[cfg(not(windows))]
#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> i32 {
    0 // S_OK
}

#[cfg(windows)]
mod win_com_exports {
    use core::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use windows::Win32::Foundation::{BOOL, CLASS_E_CLASSNOTAVAILABLE, E_NOINTERFACE, HMODULE, S_OK};
    use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
    use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
    use windows::Win32::UI::TextServices::ITfTextInputProcessorEx;
    use windows_core::{GUID, IUnknown, Interface, implement};

    use crate::registration::CLSID_VNTYPE_TEXT_SERVICE;
    use crate::registration::win_reg::{register_tsf_server, unregister_tsf_server};
    use crate::text_service::win_tsf::WinTextService;
    use crate::{decrement_dll_ref, increment_dll_ref};

    static DLL_HMODULE: AtomicUsize = AtomicUsize::new(0);

    #[implement(IClassFactory)]
    struct WinClassFactory;

    impl IClassFactory_Impl for WinClassFactory_Impl {
        fn CreateInstance(
            &self,
            punkouter: windows_core::Ref<'_, IUnknown>,
            riid: *const GUID,
            ppvobject: *mut *mut c_void,
        ) -> windows_core::Result<()> {
            if ppvobject.is_null() {
                return Err(windows::Win32::Foundation::E_POINTER.into());
            }
            unsafe { *ppvobject = core::ptr::null_mut() };
            if !punkouter.is_null() {
                return Err(windows::Win32::Foundation::CLASS_E_NOAGGREGATION.into());
            }
            let tip: ITfTextInputProcessorEx = WinTextService::new().into();
            unsafe { tip.query(riid, ppvobject).ok() }
        }

        fn LockServer(&self, flock: BOOL) -> windows_core::Result<()> {
            if flock.as_bool() {
                increment_dll_ref();
            } else {
                decrement_dll_ref();
            }
            Ok(())
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn DllMain(hmodule: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
        if reason == DLL_PROCESS_ATTACH {
            DLL_HMODULE.store(hmodule.0 as usize, Ordering::SeqCst);
        }
        BOOL::from(true)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "system" fn DllGetClassObject(
        rclsid: *const GUID,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> i32 {
        if ppv.is_null() {
            return windows::Win32::Foundation::E_POINTER.0;
        }
        unsafe { *ppv = core::ptr::null_mut() };
        if rclsid.is_null() || riid.is_null() {
            return E_NOINTERFACE.0;
        }
        let target = GUID::from_u128(CLSID_VNTYPE_TEXT_SERVICE);
        if unsafe { *rclsid } != target {
            return CLASS_E_CLASSNOTAVAILABLE.0;
        }
        let factory: IClassFactory = WinClassFactory.into();
        unsafe { factory.query(riid, ppv).0 }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn DllRegisterServer() -> i32 {
        let hmod = HMODULE(DLL_HMODULE.load(Ordering::SeqCst) as *mut c_void);
        match register_tsf_server(hmod) {
            Ok(()) => S_OK.0,
            Err(e) => e.code().0,
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn DllUnregisterServer() -> i32 {
        match unregister_tsf_server() {
            Ok(()) => S_OK.0,
            Err(e) => e.code().0,
        }
    }
}
