//! Quản lý biểu tượng khay hệ thống (System Tray Icon) và thực đơn ngữ cảnh nhanh trên Windows.

/// Các lệnh điều khiển người dùng có thể kích hoạt từ biểu tượng khay hệ thống.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    /// Bật hoặc tắt chế độ gõ tiếng Việt (`Alt+,`).
    ToggleVietnamese,
    /// Chọn kiểu gõ Telex.
    SelectTelex,
    /// Chọn kiểu gõ VNI.
    SelectVni,
    /// Chọn kiểu gõ Simple Telex.
    SelectSimpleTelex,
    /// Mở cửa sổ giao diện cấu hình `vntype-settings`.
    OpenSettingsWindow,
    /// Thoát trình nền VNType.
    ExitApplication,
}

/// Trạng thái hiển thị của biểu tượng khay hệ thống.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayStatus {
    /// `true` nếu đang ở chế độ Tiếng Việt (hiển thị nhãn `VN`), `false` nếu Tiếng Anh (`EN`).
    pub vietnamese_enabled: bool,
    /// Tên kiểu gõ hiện tại (`"Telex"`, `"VNI"`, ...).
    pub input_method_label: String,
}

impl TrayStatus {
    /// Tạo chuỗi chú thích (Tooltip) hiển thị khi di chuột lên biểu tượng khay hệ thống.
    pub fn tooltip_text(&self) -> String {
        let mode = if self.vietnamese_enabled { "Tiếng Việt (VN)" } else { "Tiếng Anh (EN)" };
        format!("VNType — {} | Kiểu gõ: {} (Alt+, để chuyển)", mode, self.input_method_label)
    }
}

#[cfg(windows)]
pub mod win_tray {
    use std::cell::RefCell;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
        GetCursorPos, IDI_APPLICATION, LoadIconW, MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
        PostQuitMessage, RegisterClassW, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY,
        TPM_RETURNCMD, TrackPopupMenu, WM_APP, WM_LBUTTONDBLCLK, WM_RBUTTONUP, WNDCLASSW,
    };

    use super::{TrayMenuAction, TrayStatus};

    const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
    const TRAY_ICON_ID: u32 = 1;

    const CMD_TOGGLE_VN: usize = 1001;
    const CMD_TELEX: usize = 1002;
    const CMD_VNI: usize = 1003;
    const CMD_SIMPLE_TELEX: usize = 1004;
    const CMD_SETTINGS: usize = 1005;
    const CMD_EXIT: usize = 1006;

    thread_local! {
        static TRAY_STATE: RefCell<TrayStatus> = RefCell::new(TrayStatus {
            vietnamese_enabled: true,
            input_method_label: "Telex".into(),
        });
        static PENDING_ACTION: RefCell<Option<TrayMenuAction>> = const { RefCell::new(None) };
    }

    fn to_wide(s: &str) -> Vec<u16> {
        let mut w: Vec<u16> = s.encode_utf16().collect();
        w.push(0);
        w
    }

    fn fill_tip(buf: &mut [u16; 128], text: &str) {
        buf.fill(0);
        for (dst, src) in buf.iter_mut().take(127).zip(text.encode_utf16()) {
            *dst = src;
        }
    }

    /// Biểu tượng khay hệ thống Win32 (`Shell_NotifyIconW`).
    pub struct WinTrayIcon {
        hwnd: HWND,
    }

    impl WinTrayIcon {
        /// Tạo cửa sổ ẩn và đăng ký biểu tượng khay hệ thống.
        pub fn new(initial: TrayStatus) -> Option<Self> {
            TRAY_STATE.with(|cell| *cell.borrow_mut() = initial.clone());
            unsafe {
                let hinstance = GetModuleHandleW(core::ptr::null());
                let class_name = to_wide("VNTypeTrayWindow");
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(tray_wnd_proc),
                    hInstance: hinstance,
                    lpszClassName: class_name.as_ptr(),
                    ..core::mem::zeroed()
                };
                RegisterClassW(&wc);

                let hwnd = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    to_wide("VNType Tray").as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    hinstance,
                    core::ptr::null(),
                );
                if hwnd.is_null() {
                    return None;
                }

                let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = TRAY_ICON_ID;
                nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
                nid.uCallbackMessage = WM_TRAY_CALLBACK;
                nid.hIcon = LoadIconW(core::ptr::null_mut(), IDI_APPLICATION);
                fill_tip(&mut nid.szTip, &initial.tooltip_text());
                Shell_NotifyIconW(NIM_ADD, &mut nid);

                Some(Self { hwnd })
            }
        }

        /// Cập nhật tooltip và trạng thái menu của biểu tượng khay hệ thống.
        pub fn update(&self, status: TrayStatus) {
            let tip = status.tooltip_text();
            TRAY_STATE.with(|cell| *cell.borrow_mut() = status);
            unsafe {
                let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = self.hwnd;
                nid.uID = TRAY_ICON_ID;
                nid.uFlags = NIF_TIP;
                fill_tip(&mut nid.szTip, &tip);
                Shell_NotifyIconW(NIM_MODIFY, &mut nid);
            }
        }

        /// Lấy hành động người dùng vừa chọn từ menu khay hệ thống (nếu có).
        pub fn take_pending_action(&self) -> Option<TrayMenuAction> {
            PENDING_ACTION.with(|cell| cell.borrow_mut().take())
        }
    }

    impl Drop for WinTrayIcon {
        fn drop(&mut self) {
            if !self.hwnd.is_null() {
                unsafe {
                    let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                    nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
                    nid.hWnd = self.hwnd;
                    nid.uID = TRAY_ICON_ID;
                    Shell_NotifyIconW(NIM_DELETE, &mut nid);
                    DestroyWindow(self.hwnd);
                }
            }
        }
    }

    unsafe fn show_context_menu(hwnd: HWND) {
        let state = TRAY_STATE.with(|cell| cell.borrow().clone());
        let menu = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return;
        }

        let vn_flag = if state.vietnamese_enabled { MF_CHECKED } else { MF_UNCHECKED };
        let telex_flag = if state.input_method_label == "Telex" { MF_CHECKED } else { MF_UNCHECKED };
        let vni_flag = if state.input_method_label == "VNI" { MF_CHECKED } else { MF_UNCHECKED };
        let simple_flag =
            if state.input_method_label == "Simple Telex" { MF_CHECKED } else { MF_UNCHECKED };

        unsafe {
            AppendMenuW(
                menu,
                MF_STRING | vn_flag,
                CMD_TOGGLE_VN,
                to_wide("Bật gõ Tiếng Việt (Alt+,)").as_ptr(),
            );
            AppendMenuW(menu, MF_SEPARATOR, 0, core::ptr::null());
            AppendMenuW(menu, MF_STRING | telex_flag, CMD_TELEX, to_wide("Kiểu gõ Telex").as_ptr());
            AppendMenuW(menu, MF_STRING | vni_flag, CMD_VNI, to_wide("Kiểu gõ VNI").as_ptr());
            AppendMenuW(
                menu,
                MF_STRING | simple_flag,
                CMD_SIMPLE_TELEX,
                to_wide("Kiểu gõ Simple Telex").as_ptr(),
            );
            AppendMenuW(menu, MF_SEPARATOR, 0, core::ptr::null());
            AppendMenuW(menu, MF_STRING, CMD_SETTINGS, to_wide("Cài đặt VNType...").as_ptr());
            AppendMenuW(menu, MF_STRING, CMD_EXIT, to_wide("Thoát").as_ptr());

            let mut pt: POINT = core::mem::zeroed();
            GetCursorPos(&mut pt);
            SetForegroundWindow(hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
                pt.x,
                pt.y,
                0,
                hwnd,
                core::ptr::null(),
            ) as usize;
            DestroyMenu(menu);

            let action = match cmd {
                CMD_TOGGLE_VN => Some(TrayMenuAction::ToggleVietnamese),
                CMD_TELEX => Some(TrayMenuAction::SelectTelex),
                CMD_VNI => Some(TrayMenuAction::SelectVni),
                CMD_SIMPLE_TELEX => Some(TrayMenuAction::SelectSimpleTelex),
                CMD_SETTINGS => Some(TrayMenuAction::OpenSettingsWindow),
                CMD_EXIT => {
                    PostQuitMessage(0);
                    Some(TrayMenuAction::ExitApplication)
                }
                _ => None,
            };
            if let Some(act) = action {
                PENDING_ACTION.with(|cell| *cell.borrow_mut() = Some(act));
            }
        }
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_TRAY_CALLBACK {
            let event = (lparam & 0xFFFF) as u32;
            match event {
                WM_LBUTTONDBLCLK => {
                    PENDING_ACTION.with(|cell| {
                        *cell.borrow_mut() = Some(TrayMenuAction::ToggleVietnamese);
                    });
                    return 0;
                }
                WM_RBUTTONUP => {
                    unsafe { show_context_menu(hwnd) };
                    return 0;
                }
                _ => {}
            }
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_tooltip_contains_mode_and_method() {
        let s = TrayStatus { vietnamese_enabled: true, input_method_label: "Telex".into() };
        assert!(s.tooltip_text().contains("VN") && s.tooltip_text().contains("Telex"));
    }
}
