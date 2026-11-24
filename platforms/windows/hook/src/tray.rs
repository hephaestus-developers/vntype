//! Quản lý biểu tượng khay hệ thống (System Tray Icon) và thực đơn ngữ cảnh nhanh trên Windows.

use vntype_config::SwitchKey;

/// Các lệnh điều khiển người dùng có thể kích hoạt từ biểu tượng khay hệ thống.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    /// Bật hoặc tắt chế độ gõ tiếng Việt.
    ToggleVietnamese,
    /// Chọn kiểu gõ Telex.
    SelectTelex,
    /// Chọn kiểu gõ VNI.
    SelectVni,
    /// Chọn kiểu gõ Simple Telex.
    SelectSimpleTelex,
    /// Chọn kiểu gõ VIQR.
    SelectViqr,
    /// Chọn bảng mã đầu ra.
    SelectCharset(&'static str),
    /// Mở cửa sổ giao diện cấu hình `vntype-settings`.
    OpenSettingsWindow,
    /// Thoát trình nền VNType.
    ExitApplication,
}

/// Trạng thái hiển thị của biểu tượng khay hệ thống.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayStatus {
    /// `true` nếu đang ở chế độ Tiếng Việt (hiển thị nhãn `V`), `false` nếu Tiếng Anh (`E`).
    pub vietnamese_enabled: bool,
    /// Tên kiểu gõ hiện tại (`"Telex"`, `"VNI"`, ...).
    pub input_method_label: String,
    /// Định danh bảng mã đầu ra hiện tại (`"unicode"`, `"tcvn3"`, ...).
    pub charset: String,
    /// Phím tắt chuyển chế độ hiện tại.
    pub switch_key: SwitchKey,
}

impl TrayStatus {
    /// Nhãn phím tắt hiện tại (`"Alt+,"` hoặc `"Alt+Z"`).
    pub fn switch_key_label(&self) -> &'static str {
        match self.switch_key {
            SwitchKey::CtrlShift => "Ctrl+Shift",
            SwitchKey::AltComma => "Alt+,",
            SwitchKey::AltZ => "Alt+Z",
        }
    }

    /// Tạo chuỗi chú thích (Tooltip) hiển thị khi di chuột lên biểu tượng khay hệ thống.
    pub fn tooltip_text(&self) -> String {
        let mode = if self.vietnamese_enabled { "Tiếng Việt (VN)" } else { "Tiếng Anh (EN)" };
        format!(
            "VNType — {} | Kiểu gõ: {} ({} để chuyển)",
            mode,
            self.input_method_label,
            self.switch_key_label()
        )
    }
}

#[cfg(windows)]
pub mod win_tray {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateBitmap, CreateCompatibleBitmap, CreateCompatibleDC,
        CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_SINGLELINE,
        DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FF_SWISS, FW_BOLD, FillRect, GetDC,
        OUT_DEFAULT_PRECIS, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
    };
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_NOSOUND, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
        DestroyMenu, DestroyWindow, GetCursorPos, HICON, ICONINFO, IDI_APPLICATION, LoadIconW, MF_CHECKED,
        MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, PostMessageW, PostQuitMessage, RegisterClassW,
        SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TrackPopupMenu,
        WM_APP, WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
    };

    use vntype_config::SwitchKey;

    use super::{TrayMenuAction, TrayStatus};

    const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
    /// Thông điệp đánh thức vòng lặp thông điệp để đồng bộ cấu hình và thao tác khay.
    pub const WM_WAKE_SYNC: u32 = WM_APP + 2;
    /// Thông điệp yêu cầu luồng Tray/Hook thoát vòng lặp `GetMessageW` an toàn.
    pub const WM_QUIT_APP: u32 = WM_APP + 3;
    /// Thông điệp yêu cầu luồng Tray/Hook chuyển đổi trạng thái bật/tắt Tiếng Việt.
    pub const WM_TOGGLE_VN: u32 = WM_APP + 4;
    /// Thông điệp yêu cầu mở cửa sổ cài đặt từ tiến trình thứ hai (single-instance).
    pub const WM_SHOW_SETTINGS: u32 = WM_APP + 5;
    const HOTKEY_SWITCH_ID: i32 = 1;
    static HOTKEY_REGISTERED: AtomicBool = AtomicBool::new(false);
    const TRAY_ICON_ID: u32 = 1;

    const CMD_TOGGLE_VN: usize = 1001;
    const CMD_TELEX: usize = 1002;
    const CMD_VNI: usize = 1003;
    const CMD_SIMPLE_TELEX: usize = 1004;
    const CMD_VIQR: usize = 1005;
    const CMD_SETTINGS: usize = 1006;
    const CMD_EXIT: usize = 1007;
    const CMD_CHARSET_BASE: usize = 2000;

    const TRAY_CHARSETS: &[(&str, &str)] = &[
        ("unicode", "Unicode"),
        ("tcvn3", "TCVN3 (ABC)"),
        ("vni_windows", "VNI Windows"),
        ("viqr", "VIQR"),
        ("cp1258", "Vietnamese locale CP 1258"),
        ("unicode_nfd", "Unicode tổ hợp"),
        ("utf8_literal", "UTF-8 Literal"),
        ("ncr_decimal", "NCR Decimal"),
        ("ncr_hex", "NCR Hex"),
        ("cstring_hex", "Unicode C String"),
        ("x_utf8", "X UTF-8"),
        ("viscii", "VISCII"),
        ("vps", "VPS"),
        ("bkhcm2", "BK HCM 2"),
        ("bkhcm1", "BK HCM 1"),
        ("vietware_x", "Vietware X"),
        ("vietware_full", "Vietware F"),
    ];

    thread_local! {
        static TRAY_STATE: RefCell<TrayStatus> = RefCell::new(TrayStatus {
            vietnamese_enabled: true,
            input_method_label: "Telex".into(),
            charset: "unicode".into(),
            switch_key: SwitchKey::AltComma,
        });
        static PENDING_ACTIONS: RefCell<Vec<TrayMenuAction>> = const { RefCell::new(Vec::new()) };
    }

    fn push_action(hwnd: HWND, action: TrayMenuAction) {
        PENDING_ACTIONS.with(|cell| cell.borrow_mut().push(action));
        unsafe {
            PostMessageW(hwnd, WM_WAKE_SYNC, 0, 0);
        }
    }

    fn to_wide(s: &str) -> Vec<u16> {
        let mut w: Vec<u16> = s.encode_utf16().collect();
        w.push(0);
        w
    }

    fn fill_wide_buf<const N: usize>(buf: &mut [u16; N], text: &str) {
        buf.fill(0);
        if N > 1 {
            for (dst, src) in buf.iter_mut().take(N - 1).zip(text.encode_utf16()) {
                *dst = src;
            }
        }
    }

    fn fill_tip(buf: &mut [u16; 128], text: &str) {
        fill_wide_buf(buf, text);
    }

    fn create_letter_icon(letter: &str, bg_color: u32, fg_color: u32) -> HICON {
        const SIZE: i32 = 32;
        unsafe {
            let screen_dc = GetDC(core::ptr::null_mut());
            let mem_dc = CreateCompatibleDC(screen_dc);
            let color_bmp = CreateCompatibleBitmap(screen_dc, SIZE, SIZE);
            let mask_bits = [0u8; (SIZE as usize * SIZE as usize) / 8];
            let mask_bmp = CreateBitmap(SIZE, SIZE, 1, 1, mask_bits.as_ptr().cast());

            let old_bmp = SelectObject(mem_dc, color_bmp);
            let brush = CreateSolidBrush(bg_color);
            let mut rect = RECT { left: 0, top: 0, right: SIZE, bottom: SIZE };
            FillRect(mem_dc, &rect, brush);
            DeleteObject(brush);

            let font_name = to_wide("Segoe UI");
            let font = CreateFontW(
                -24,
                0,
                0,
                0,
                FW_BOLD as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                (DEFAULT_PITCH | FF_SWISS) as u32,
                font_name.as_ptr(),
            );
            let old_font = SelectObject(mem_dc, font);
            SetBkMode(mem_dc, TRANSPARENT as i32);
            SetTextColor(mem_dc, fg_color);
            let text = to_wide(letter);
            DrawTextW(mem_dc, text.as_ptr(), -1, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            SelectObject(mem_dc, old_font);
            SelectObject(mem_dc, old_bmp);

            let ii =
                ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask_bmp, hbmColor: color_bmp };
            let icon = CreateIconIndirect(&ii);

            DeleteObject(font);
            DeleteObject(mask_bmp);
            DeleteObject(color_bmp);
            DeleteDC(mem_dc);
            ReleaseDC(core::ptr::null_mut(), screen_dc);

            if icon.is_null() { LoadIconW(core::ptr::null_mut(), IDI_APPLICATION) } else { icon }
        }
    }

    /// Biểu tượng khay hệ thống Win32 (`Shell_NotifyIconW`).
    pub struct WinTrayIcon {
        hwnd: HWND,
        icon_v: HICON,
        icon_e: HICON,
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

                // COLORREF định dạng 0x00BBGGRR: Đỏ cho "V", Xanh dương cho "E".
                let icon_v = create_letter_icon("V", 0x002F_2FD3, 0x00FF_FFFF);
                let icon_e = create_letter_icon("E", 0x00D2_7619, 0x00FF_FFFF);

                let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = TRAY_ICON_ID;
                nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
                nid.uCallbackMessage = WM_TRAY_CALLBACK;
                nid.hIcon = if initial.vietnamese_enabled { icon_v } else { icon_e };
                fill_tip(&mut nid.szTip, &initial.tooltip_text());
                Shell_NotifyIconW(NIM_ADD, &mut nid);

                Some(Self { hwnd, icon_v, icon_e })
            }
        }

        /// Cập nhật biểu tượng (`V`/`E`), tooltip và trạng thái menu của khay hệ thống.
        pub fn update(&self, status: TrayStatus) {
            let tip = status.tooltip_text();
            let icon = if status.vietnamese_enabled { self.icon_v } else { self.icon_e };
            TRAY_STATE.with(|cell| *cell.borrow_mut() = status);
            unsafe {
                let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = self.hwnd;
                nid.uID = TRAY_ICON_ID;
                nid.uFlags = NIF_ICON | NIF_TIP;
                nid.hIcon = icon;
                fill_tip(&mut nid.szTip, &tip);
                Shell_NotifyIconW(NIM_MODIFY, &mut nid);
            }
        }

        /// Hiển thị thông báo phản hồi dạng balloon trên khay hệ thống.
        pub fn show_balloon(&self, title: &str, msg: &str) {
            unsafe {
                let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = self.hwnd;
                nid.uID = TRAY_ICON_ID;
                nid.uFlags = NIF_INFO;
                nid.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
                fill_wide_buf(&mut nid.szInfoTitle, title);
                fill_wide_buf(&mut nid.szInfo, msg);
                Shell_NotifyIconW(NIM_MODIFY, &mut nid);
            }
        }

        /// Trả về handle cửa sổ ẩn của tray dưới dạng `isize` để gửi thông điệp điều khiển liên luồng.
        pub fn hwnd_raw(&self) -> isize {
            self.hwnd as isize
        }

        /// Đăng ký phím tắt toàn cục (`RegisterHotKey`) để hoạt động ổn định cả trong cửa sổ quyền Admin.
        pub fn register_switch_hotkey(&self, switch_key: SwitchKey) {
            unsafe {
                UnregisterHotKey(self.hwnd, HOTKEY_SWITCH_ID);
                let ok = match switch_key {
                    SwitchKey::AltZ => {
                        RegisterHotKey(self.hwnd, HOTKEY_SWITCH_ID, MOD_ALT | MOD_NOREPEAT, 0x5Au32) != 0
                    }
                    SwitchKey::CtrlShift | SwitchKey::AltComma => false,
                };
                HOTKEY_REGISTERED.store(ok, Ordering::Release);
            }
        }

        /// Kiểm tra xem `RegisterHotKey` có đang xử lý phím chuyển hay không.
        pub fn is_hotkey_registered() -> bool {
            HOTKEY_REGISTERED.load(Ordering::Acquire)
        }

        /// Lấy hành động tiếp theo người dùng vừa chọn từ menu khay hệ thống (nếu có).
        pub fn take_pending_action(&self) -> Option<TrayMenuAction> {
            PENDING_ACTIONS.with(|cell| {
                let mut q = cell.borrow_mut();
                if q.is_empty() { None } else { Some(q.remove(0)) }
            })
        }
    }

    impl Drop for WinTrayIcon {
        fn drop(&mut self) {
            unsafe {
                if !self.hwnd.is_null() {
                    UnregisterHotKey(self.hwnd, HOTKEY_SWITCH_ID);
                    HOTKEY_REGISTERED.store(false, Ordering::Release);
                    let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                    nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
                    nid.hWnd = self.hwnd;
                    nid.uID = TRAY_ICON_ID;
                    Shell_NotifyIconW(NIM_DELETE, &mut nid);
                    DestroyWindow(self.hwnd);
                }
                if !self.icon_v.is_null() {
                    DestroyIcon(self.icon_v);
                }
                if !self.icon_e.is_null() {
                    DestroyIcon(self.icon_e);
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
        let simple_flag =
            if state.input_method_label == "Simple Telex" { MF_CHECKED } else { MF_UNCHECKED };
        let vni_flag = if state.input_method_label == "VNI" { MF_CHECKED } else { MF_UNCHECKED };
        let viqr_flag = if state.input_method_label == "VIQR" { MF_CHECKED } else { MF_UNCHECKED };

        unsafe {
            let toggle_label = format!("Bật gõ Tiếng Việt ({})", state.switch_key_label());
            AppendMenuW(menu, MF_STRING | vn_flag, CMD_TOGGLE_VN, to_wide(&toggle_label).as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, core::ptr::null());

            let charset_menu = CreatePopupMenu();
            if !charset_menu.is_null() {
                for (idx, &(key, label)) in TRAY_CHARSETS.iter().enumerate() {
                    if idx == 6 || idx == 11 {
                        AppendMenuW(charset_menu, MF_SEPARATOR, 0, core::ptr::null());
                    }
                    let flag = if state.charset == key { MF_CHECKED } else { MF_UNCHECKED };
                    AppendMenuW(
                        charset_menu,
                        MF_STRING | flag,
                        CMD_CHARSET_BASE + idx,
                        to_wide(label).as_ptr(),
                    );
                }
                AppendMenuW(menu, MF_POPUP, charset_menu as usize, to_wide("Bảng mã").as_ptr());
            }

            let method_menu = CreatePopupMenu();
            if !method_menu.is_null() {
                AppendMenuW(method_menu, MF_STRING | telex_flag, CMD_TELEX, to_wide("Telex").as_ptr());
                AppendMenuW(
                    method_menu,
                    MF_STRING | simple_flag,
                    CMD_SIMPLE_TELEX,
                    to_wide("Telex giản lược").as_ptr(),
                );
                AppendMenuW(method_menu, MF_STRING | vni_flag, CMD_VNI, to_wide("VNI").as_ptr());
                AppendMenuW(method_menu, MF_STRING | viqr_flag, CMD_VIQR, to_wide("VIQR").as_ptr());
                AppendMenuW(menu, MF_POPUP, method_menu as usize, to_wide("Kiểu gõ").as_ptr());
            }

            AppendMenuW(menu, MF_SEPARATOR, 0, core::ptr::null());
            AppendMenuW(menu, MF_STRING, CMD_SETTINGS, to_wide("Bảng điều khiển...").as_ptr());
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
                CMD_VIQR => Some(TrayMenuAction::SelectViqr),
                CMD_SETTINGS => Some(TrayMenuAction::OpenSettingsWindow),
                CMD_EXIT => {
                    PostQuitMessage(0);
                    Some(TrayMenuAction::ExitApplication)
                }
                c if c >= CMD_CHARSET_BASE && c < CMD_CHARSET_BASE + TRAY_CHARSETS.len() => {
                    Some(TrayMenuAction::SelectCharset(TRAY_CHARSETS[c - CMD_CHARSET_BASE].0))
                }
                _ => None,
            };
            if let Some(act) = action {
                push_action(hwnd, act);
            }
        }
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_QUIT_APP {
            unsafe { PostQuitMessage(0) };
            return 0;
        }
        if msg == WM_TOGGLE_VN {
            push_action(hwnd, TrayMenuAction::ToggleVietnamese);
            return 0;
        }
        if msg == WM_SHOW_SETTINGS {
            push_action(hwnd, TrayMenuAction::OpenSettingsWindow);
            return 0;
        }
        if msg == WM_HOTKEY && wparam as i32 == HOTKEY_SWITCH_ID {
            push_action(hwnd, TrayMenuAction::ToggleVietnamese);
            return 0;
        }
        if msg == WM_TRAY_CALLBACK {
            let event = (lparam & 0xFFFF) as u32;
            match event {
                WM_LBUTTONUP => {
                    push_action(hwnd, TrayMenuAction::ToggleVietnamese);
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
        let s = TrayStatus {
            vietnamese_enabled: true,
            input_method_label: "Telex".into(),
            charset: "unicode".into(),
            switch_key: vntype_config::SwitchKey::AltComma,
        };
        assert!(s.tooltip_text().contains("VN") && s.tooltip_text().contains("Telex"));
    }
}
