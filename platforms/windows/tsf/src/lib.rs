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
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> i32 {
    0 // S_OK
}

/// Điểm xuất chuẩn COM `DllUnregisterServer`: gỡ đăng ký Text Service.
#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> i32 {
    0 // S_OK
}
