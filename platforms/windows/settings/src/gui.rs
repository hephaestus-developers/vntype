//! Giao diện cài đặt bản địa (Win32 API + Common Controls) cho VNType trên Windows.

pub mod win_gui {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

    use crate::SettingsViewModel;

    /// Cầu nối đồng bộ giữa luồng nền (`vntype-hook`) và cửa sổ Win32 trên luồng chính.
    pub struct HookGuiBridge {
        show_requested: AtomicBool,
        exit_requested: AtomicBool,
        refresh_requested: AtomicBool,
        vietnamese_enabled: AtomicBool,
        #[cfg_attr(not(windows), allow(dead_code))]
        hwnd: AtomicIsize,
    }

    impl Default for HookGuiBridge {
        fn default() -> Self {
            Self {
                show_requested: AtomicBool::new(false),
                exit_requested: AtomicBool::new(false),
                refresh_requested: AtomicBool::new(false),
                vietnamese_enabled: AtomicBool::new(true),
                hwnd: AtomicIsize::new(0),
            }
        }
    }

    impl HookGuiBridge {
        /// Tạo mới cầu nối dùng chung giữa luồng Hook/Tray và luồng giao diện.
        pub fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }

        fn wake(&self) {
            #[cfg(windows)]
            {
                let raw = self.hwnd.load(Ordering::Acquire);
                if raw != 0 {
                    unsafe {
                        windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                            raw as windows_sys::Win32::Foundation::HWND,
                            win32_impl::WM_BRIDGE_WAKE,
                            0,
                            0,
                        );
                    }
                }
            }
        }

        /// Yêu cầu hiển thị và đưa cửa sổ cấu hình lên phía trước.
        pub fn request_show(&self) {
            self.show_requested.store(true, Ordering::Release);
            #[cfg(windows)]
            unsafe {
                let raw = self.hwnd.load(Ordering::Acquire);
                if raw != 0 {
                    let hwnd = raw as windows_sys::Win32::Foundation::HWND;
                    windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                        hwnd,
                        windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
                    );
                    windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
                }
            }
            self.wake();
        }

        /// Yêu cầu thoát hoàn toàn ứng dụng GUI.
        pub fn request_exit(&self) {
            self.exit_requested.store(true, Ordering::Release);
            self.wake();
        }

        /// Báo cho GUI nạp lại cấu hình từ đĩa (khi người dùng đổi kiểu gõ từ menu khay).
        pub fn request_refresh(&self) {
            self.refresh_requested.store(true, Ordering::Release);
            self.wake();
        }

        /// Cập nhật trạng thái bật/tắt Tiếng Việt từ luồng Hook/Tray để hiển thị lên GUI.
        pub fn set_vietnamese_enabled(&self, enabled: bool) {
            self.vietnamese_enabled.store(enabled, Ordering::Release);
            self.wake();
        }

        /// Đọc trạng thái bật/tắt Tiếng Việt hiện tại.
        pub fn is_vietnamese_enabled(&self) -> bool {
            self.vietnamese_enabled.load(Ordering::Acquire)
        }
    }

    #[cfg(windows)]
    mod win32_impl {
        use std::cell::RefCell;
        use std::ptr::{null, null_mut};
        use std::sync::Arc;
        use std::sync::atomic::Ordering;

        use vntype_config::{SwitchKey, ToneStyle};
        use vntype_core::{CHARSETS, charset_display_name};
        use vntype_settings_core::{PlatformCapabilities, SettingsViewModel};
        use vntype_win_ipc::win_shm::SharedRoutingMapping;
        use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
        use windows_sys::Win32::Graphics::Gdi::{
            CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_BTNFACE, CreateFontW, DEFAULT_CHARSET,
            DEFAULT_PITCH, DeleteObject, FF_SWISS, FW_NORMAL, HFONT, InvalidateRect, OUT_DEFAULT_PRECIS,
        };
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::System::Registry::{
            HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
            RegQueryValueExW, RegSetValueExW,
        };
        use windows_sys::Win32::UI::Controls::{
            ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
        };
        use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_AUTORADIOBUTTON, BS_DEFPUSHBUTTON, BS_GROUPBOX,
            BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST, CreateWindowExW,
            DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetMessageW, GetSystemMetrics,
            GetWindowLongPtrW, HICON, IDC_ARROW, IMAGE_ICON, LR_DEFAULTCOLOR, LoadCursorW, LoadImageW,
            MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MSG, MessageBoxW, PostQuitMessage, RegisterClassExW,
            SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, SW_HIDE, SW_SHOWNORMAL, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
            SetWindowTextW, ShowWindow, TranslateMessage, WM_APP, WM_CLOSE, WM_COMMAND, WM_DESTROY,
            WM_DPICHANGED, WM_SETFONT, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_GROUP, WS_MINIMIZEBOX,
            WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
        };

        pub const WM_BRIDGE_WAKE: u32 = WM_APP + 1;

        const INPUT_METHODS: &[(&str, &str)] =
            &[("telex", "Telex"), ("simple_telex", "Simple Telex"), ("vni", "VNI"), ("viqr", "VIQR")];

        const ID_CHARSET: i32 = 101;
        const ID_INPUT_METHOD: i32 = 102;
        const ID_SWITCH_COMMA: i32 = 103;
        const ID_SWITCH_Z: i32 = 104;
        const ID_TONE_NEW: i32 = 105;
        const ID_TONE_OLD: i32 = 106;

        const ID_CHK_DEFAULT_VI: i32 = 201;
        const ID_CHK_SPELL: i32 = 202;
        const ID_CHK_W_AS_U: i32 = 203;
        const ID_CHK_BRACKETS: i32 = 204;
        const ID_CHK_QUICK_TELEX: i32 = 205;
        const ID_CHK_CODA_K: i32 = 206;
        const ID_CHK_EDIT_PREV: i32 = 207;
        const ID_CHK_AUTO_CAP: i32 = 208;
        const ID_CHK_SMART_CAPS: i32 = 209;
        const ID_CHK_FREE_TYPING: i32 = 210;
        const ID_CHK_AUTO_RESTORE: i32 = 211;
        const ID_CHK_CLIPBOARD_UNICODE: i32 = 212;
        const ID_CHK_FEEDBACK_NOTIFY: i32 = 213;
        const ID_CHK_BEEP_SWITCH: i32 = 214;

        const ID_CHK_MACRO_ENABLED: i32 = 301;
        const ID_CHK_MACRO_UNDO: i32 = 302;
        const ID_CHK_MACRO_DIGITS: i32 = 303;
        const ID_CHK_MACRO_OFF: i32 = 304;
        const ID_BTN_OPEN_MACROS: i32 = 305;

        const ID_CHK_AUTOSTART: i32 = 401;
        const ID_BTN_TOGGLE_MODE: i32 = 402;
        const ID_CHK_SHOW_ON_START: i32 = 404;
        const ID_CHK_PER_APP_MODE: i32 = 405;
        const ID_CHK_VIET_UI: i32 = 406;

        const ID_BTN_INFO: i32 = 501;
        const ID_BTN_DEFAULTS: i32 = 502;
        const ID_BTN_REVERT: i32 = 503;
        const ID_BTN_SAVE_CLOSE: i32 = 504;
        const ID_BTN_EXIT: i32 = 505;

        struct Controls {
            charset: HWND,
            input_method: HWND,
            switch_comma: HWND,
            switch_z: HWND,
            tone_new: HWND,
            tone_old: HWND,
            chk_default_vi: HWND,
            chk_free_typing: HWND,
            chk_clipboard_unicode: HWND,
            chk_spell: HWND,
            chk_auto_restore: HWND,
            chk_feedback_notify: HWND,
            chk_beep_switch: HWND,
            chk_w_as_u: HWND,
            chk_brackets: HWND,
            chk_quick_telex: HWND,
            chk_coda_k: HWND,
            chk_edit_prev: HWND,
            chk_auto_cap: HWND,
            chk_smart_caps: HWND,
            chk_macro_enabled: HWND,
            chk_macro_undo: HWND,
            chk_macro_digits: HWND,
            chk_macro_off: HWND,
            lbl_macro_count: HWND,
            chk_show_on_start: HWND,
            chk_autostart: HWND,
            chk_per_app_mode: HWND,
            chk_viet_ui: HWND,
            btn_toggle_mode: HWND,
            lbl_ipc_status: HWND,
            btn_revert: HWND,
        }

        struct WindowState {
            vm: SettingsViewModel,
            caps: PlatformCapabilities,
            routing: Option<SharedRoutingMapping>,
            bridge: Option<Arc<super::HookGuiBridge>>,
            on_toggle: Option<Box<dyn Fn() + Send>>,
            on_config_changed: Option<Box<dyn Fn() + Send>>,
            on_quit: Option<Box<dyn Fn() + Send>>,
            allow_close: bool,
            font: HFONT,
            controls: Option<Controls>,
            ctrl_layouts: Vec<(HWND, i32, i32, i32, i32)>,
            i18n_ctrls: Vec<(HWND, &'static str, &'static str)>,
        }

        thread_local! {
            static PENDING_LAYOUTS: RefCell<Vec<(HWND, i32, i32, i32, i32)>> = const { RefCell::new(Vec::new()) };
        }

        fn to_wide(s: &str) -> Vec<u16> {
            let mut w: Vec<u16> = s.encode_utf16().collect();
            w.push(0);
            w
        }

        static DPI_SCALE_NUMERATOR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(96);

        fn set_window_dpi(dpi: u32) {
            DPI_SCALE_NUMERATOR.store(dpi.max(96), Ordering::Release);
        }

        fn current_window_dpi() -> u32 {
            DPI_SCALE_NUMERATOR.load(Ordering::Acquire).max(96)
        }

        fn scale_px(v: i32) -> i32 {
            let dpi = current_window_dpi() as f32;
            ((v as f32) * (dpi / 96.0)).round() as i32
        }

        unsafe fn create_ui_font() -> HFONT {
            unsafe {
                CreateFontW(
                    -scale_px(12),
                    0,
                    0,
                    0,
                    FW_NORMAL as i32,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_SWISS) as u32,
                    to_wide("Segoe UI").as_ptr(),
                )
            }
        }

        unsafe fn apply_dpi_layout(
            hwnd: HWND,
            state: &mut WindowState,
            new_dpi: u32,
            suggested_rect: Option<&RECT>,
        ) {
            let new_dpi = new_dpi.max(96);
            set_window_dpi(new_dpi);

            let new_font = unsafe { create_ui_font() };
            if !new_font.is_null() {
                let old_font = state.font;
                state.font = new_font;
                for &(ctrl, _, _, _, _) in &state.ctrl_layouts {
                    unsafe { SendMessageW(ctrl, WM_SETFONT, new_font as WPARAM, 0) };
                }
                if !old_font.is_null() {
                    unsafe { DeleteObject(old_font as _) };
                }
            }

            for &(ctrl, x, y, w, h) in &state.ctrl_layouts {
                unsafe {
                    SetWindowPos(
                        ctrl,
                        null_mut(),
                        scale_px(x),
                        scale_px(y),
                        scale_px(w),
                        scale_px(h),
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }

            let mut rect = RECT { left: 0, top: 0, right: scale_px(584), bottom: scale_px(548) };
            let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
            unsafe { AdjustWindowRectExForDpi(&mut rect, style, 0, 0, new_dpi) };
            let (wx, wy, flags) = match suggested_rect {
                Some(r) => (r.left, r.top, SWP_NOZORDER | SWP_NOACTIVATE),
                None => (0, 0, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE),
            };
            unsafe {
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    wx,
                    wy,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    flags,
                );
                InvalidateRect(hwnd, null(), 1);
            }
        }

        unsafe fn create_ctrl(
            parent: HWND,
            class: &str,
            text: &str,
            style: u32,
            ex_style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            id: i32,
            font: HFONT,
        ) -> HWND {
            let cls = to_wide(class);
            let txt = to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    ex_style,
                    cls.as_ptr(),
                    txt.as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    scale_px(x),
                    scale_px(y),
                    scale_px(w),
                    scale_px(h),
                    parent,
                    id as _,
                    GetModuleHandleW(null()),
                    null(),
                )
            };
            if !hwnd.is_null() {
                if !font.is_null() {
                    unsafe { SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1) };
                }
                PENDING_LAYOUTS.with(|layouts| layouts.borrow_mut().push((hwnd, x, y, w, h)));
            }
            hwnd
        }

        unsafe fn set_checked(hwnd: HWND, checked: bool) {
            unsafe { SendMessageW(hwnd, BM_SETCHECK, if checked { 1 } else { 0 }, 0) };
        }

        unsafe fn is_checked(hwnd: HWND) -> bool {
            unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) != 0 }
        }

        fn is_autostart_enabled() -> bool {
            let subkey = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
            let val_name = to_wide("VNType");
            let mut hkey = null_mut();
            unsafe {
                if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
                    return false;
                }
                let ok = RegQueryValueExW(
                    hkey,
                    val_name.as_ptr(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                ) == 0;
                RegCloseKey(hkey);
                ok
            }
        }

        fn set_autostart(enabled: bool) {
            let subkey = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
            let val_name = to_wide("VNType");
            let mut hkey = null_mut();
            unsafe {
                if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut hkey) != 0 {
                    return;
                }
                if enabled {
                    if let Ok(exe) = std::env::current_exe() {
                        let cmd = format!("\"{}\"", exe.display());
                        let wcmd = to_wide(&cmd);
                        let bytes = (wcmd.len() * 2) as u32;
                        let _ = RegSetValueExW(
                            hkey,
                            val_name.as_ptr(),
                            0,
                            REG_SZ,
                            wcmd.as_ptr() as *const u8,
                            bytes,
                        );
                    }
                } else {
                    let _ = RegDeleteValueW(hkey, val_name.as_ptr());
                }
                RegCloseKey(hkey);
            }
        }

        fn apply_interface_language(state: &WindowState) {
            let vi = state.vm.draft.vietnamese_interface;
            unsafe {
                for &(ctrl, vi_txt, en_txt) in &state.i18n_ctrls {
                    let w = to_wide(if vi { vi_txt } else { en_txt });
                    SetWindowTextW(ctrl, w.as_ptr());
                }
            }
            if let Some(ref c) = state.controls {
                let count = state.vm.macro_entry_count();
                let count_str =
                    if vi { format!("({count} quy tắc)") } else { format!("({count} rules)") };
                unsafe {
                    SetWindowTextW(c.lbl_macro_count, to_wide(&count_str).as_ptr());
                }
            }
            update_bridge_labels(state);
        }

        fn sync_ui_from_vm(state: &WindowState) {
            let Some(ref c) = state.controls else { return };
            let d = &state.vm.draft;
            unsafe {
                let cs_idx = CHARSETS.iter().position(|&k| k == d.charset).unwrap_or(0);
                SendMessageW(c.charset, CB_SETCURSEL, cs_idx, 0);

                let im_idx = INPUT_METHODS.iter().position(|&(k, _)| k == d.input_method).unwrap_or(0);
                SendMessageW(c.input_method, CB_SETCURSEL, im_idx, 0);

                set_checked(c.switch_comma, d.switch_key != SwitchKey::AltZ);
                set_checked(c.switch_z, d.switch_key == SwitchKey::AltZ);
                set_checked(c.tone_new, d.tone_style == ToneStyle::New);
                set_checked(c.tone_old, d.tone_style == ToneStyle::Old);

                set_checked(c.chk_default_vi, d.default_vietnamese);
                set_checked(c.chk_free_typing, d.free_typing);
                set_checked(c.chk_clipboard_unicode, d.always_use_clipboard_unicode);
                set_checked(c.chk_spell, d.spell_check);
                set_checked(c.chk_auto_restore, d.auto_restore_keys);
                set_checked(c.chk_feedback_notify, d.show_feedback_notification);
                set_checked(c.chk_beep_switch, d.beep_on_switch);
                set_checked(c.chk_w_as_u, d.w_as_u);
                set_checked(c.chk_brackets, d.bracket_keys);
                set_checked(c.chk_quick_telex, d.quick_telex);
                set_checked(c.chk_coda_k, d.allow_coda_k);
                set_checked(c.chk_edit_prev, d.edit_previous_word);
                set_checked(c.chk_auto_cap, d.auto_capitalize);
                set_checked(c.chk_smart_caps, d.smart_caps);

                set_checked(c.chk_macro_enabled, d.macro_enabled);
                set_checked(c.chk_macro_undo, d.macro_undo);
                set_checked(c.chk_macro_digits, d.macro_digits);
                set_checked(c.chk_macro_off, d.macro_in_off_mode);

                set_checked(c.chk_show_on_start, d.show_dialog_on_startup);
                if state.caps.supports_autostart && !c.chk_autostart.is_null() {
                    set_checked(c.chk_autostart, is_autostart_enabled());
                }
                set_checked(c.chk_per_app_mode, d.per_app_mode);
                set_checked(c.chk_viet_ui, d.vietnamese_interface);

                apply_interface_language(state);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    c.btn_revert,
                    i32::from(state.vm.is_dirty()),
                );
            }
        }

        fn update_bridge_labels(state: &WindowState) {
            let Some(ref c) = state.controls else { return };
            let vi_ui = state.vm.draft.vietnamese_interface;
            unsafe {
                if !c.btn_toggle_mode.is_null() {
                    let vi = state
                        .bridge
                        .as_ref()
                        .map(|b| b.is_vietnamese_enabled())
                        .unwrap_or(state.vm.draft.default_vietnamese);
                    let lbl = to_wide(if vi_ui {
                        if vi { "Đang bật: Tiếng Việt" } else { "Đang tắt: Tiếng Anh" }
                    } else if vi {
                        "Active: Vietnamese"
                    } else {
                        "Active: English"
                    });
                    SetWindowTextW(c.btn_toggle_mode, lbl.as_ptr());
                }
                if !c.lbl_ipc_status.is_null() {
                    let tsf_active = state
                        .routing
                        .as_ref()
                        .map(|r| {
                            r.tsf_active.load(Ordering::Acquire) == 1
                                && r.active_pid.load(Ordering::Acquire) != 0
                        })
                        .unwrap_or(false);
                    let txt = to_wide(if vi_ui {
                        if tsf_active { "TSF: Hoạt động" } else { "Hook: Portable" }
                    } else if tsf_active {
                        "TSF: Active"
                    } else {
                        "Hook: Portable"
                    });
                    SetWindowTextW(c.lbl_ipc_status, txt.as_ptr());
                }
            }
        }

        fn sync_vm_from_ui(state: &mut WindowState) {
            let Some(ref c) = state.controls else { return };
            unsafe {
                let cs_idx = SendMessageW(c.charset, CB_GETCURSEL, 0, 0) as usize;
                if let Some(&key) = CHARSETS.get(cs_idx) {
                    state.vm.set_charset(key);
                }
                let im_idx = SendMessageW(c.input_method, CB_GETCURSEL, 0, 0) as usize;
                if let Some(&(key, _)) = INPUT_METHODS.get(im_idx) {
                    state.vm.set_input_method(key);
                }

                state.vm.draft.switch_key =
                    if is_checked(c.switch_z) { SwitchKey::AltZ } else { SwitchKey::CtrlShift };
                state.vm.set_tone_style(if is_checked(c.tone_new) {
                    ToneStyle::New
                } else {
                    ToneStyle::Old
                });

                state.vm.draft.default_vietnamese = is_checked(c.chk_default_vi);
                state.vm.draft.free_typing = is_checked(c.chk_free_typing);
                state.vm.draft.always_use_clipboard_unicode = is_checked(c.chk_clipboard_unicode);
                state.vm.draft.spell_check = is_checked(c.chk_spell);
                state.vm.draft.auto_restore_keys = is_checked(c.chk_auto_restore);
                state.vm.draft.show_feedback_notification = is_checked(c.chk_feedback_notify);
                state.vm.draft.beep_on_switch = is_checked(c.chk_beep_switch);
                state.vm.draft.w_as_u = is_checked(c.chk_w_as_u);
                state.vm.draft.bracket_keys = is_checked(c.chk_brackets);
                state.vm.draft.quick_telex = is_checked(c.chk_quick_telex);
                state.vm.draft.allow_coda_k = is_checked(c.chk_coda_k);
                state.vm.draft.edit_previous_word = is_checked(c.chk_edit_prev);
                state.vm.draft.auto_capitalize = is_checked(c.chk_auto_cap);
                state.vm.draft.smart_caps = is_checked(c.chk_smart_caps);

                state.vm.draft.macro_enabled = is_checked(c.chk_macro_enabled);
                state.vm.draft.macro_undo = is_checked(c.chk_macro_undo);
                state.vm.draft.macro_digits = is_checked(c.chk_macro_digits);
                state.vm.draft.macro_in_off_mode = is_checked(c.chk_macro_off);

                state.vm.draft.show_dialog_on_startup = is_checked(c.chk_show_on_start);
                state.vm.draft.per_app_mode = is_checked(c.chk_per_app_mode);
                state.vm.draft.vietnamese_interface = is_checked(c.chk_viet_ui);

                windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    c.btn_revert,
                    i32::from(state.vm.is_dirty()),
                );
            }
        }

        fn build_controls(hwnd: HWND, state: &mut WindowState) {
            PENDING_LAYOUTS.with(|layouts| layouts.borrow_mut().clear());
            state.i18n_ctrls.clear();
            let mut i18n = |ctrl: HWND, vi: &'static str, en: &'static str| -> HWND {
                if !ctrl.is_null() {
                    state.i18n_ctrls.push((ctrl, vi, en));
                }
                ctrl
            };
            let f = state.font;
            unsafe {
                // 1. Khung Điều khiển
                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Điều khiển",
                        BS_GROUPBOX as u32,
                        0,
                        12,
                        10,
                        560,
                        118,
                        0,
                        f,
                    ),
                    "Điều khiển",
                    "Control",
                );
                i18n(
                    create_ctrl(hwnd, "STATIC", "Bảng mã:", 0, 0, 24, 34, 85, 20, 0, f),
                    "Bảng mã:",
                    "Charset:",
                );
                let charset = create_ctrl(
                    hwnd,
                    "COMBOBOX",
                    "",
                    (CBS_DROPDOWNLIST as u32) | WS_TABSTOP | WS_VSCROLL,
                    0,
                    112,
                    30,
                    190,
                    220,
                    ID_CHARSET,
                    f,
                );
                for &key in CHARSETS {
                    let label = to_wide(charset_display_name(key).unwrap_or(key));
                    SendMessageW(charset, CB_ADDSTRING, 0, label.as_ptr() as LPARAM);
                }

                i18n(
                    create_ctrl(hwnd, "STATIC", "Kiểu gõ:", 0, 0, 318, 34, 65, 20, 0, f),
                    "Kiểu gõ:",
                    "Input method:",
                );
                let input_method = create_ctrl(
                    hwnd,
                    "COMBOBOX",
                    "",
                    (CBS_DROPDOWNLIST as u32) | WS_TABSTOP | WS_VSCROLL,
                    0,
                    386,
                    30,
                    172,
                    140,
                    ID_INPUT_METHOD,
                    f,
                );
                for &(_, label) in INPUT_METHODS {
                    let wlabel = to_wide(label);
                    SendMessageW(input_method, CB_ADDSTRING, 0, wlabel.as_ptr() as LPARAM);
                }

                i18n(
                    create_ctrl(hwnd, "STATIC", "Phím chuyển:", 0, 0, 24, 66, 85, 20, 0, f),
                    "Phím chuyển:",
                    "Switch key:",
                );
                let switch_comma = create_ctrl(
                    hwnd,
                    "BUTTON",
                    "Ctrl + Shift",
                    (BS_AUTORADIOBUTTON as u32) | WS_GROUP | WS_TABSTOP,
                    0,
                    112,
                    64,
                    100,
                    22,
                    ID_SWITCH_COMMA,
                    f,
                );
                let switch_z = create_ctrl(
                    hwnd,
                    "BUTTON",
                    "Alt + Z",
                    (BS_AUTORADIOBUTTON as u32) | WS_TABSTOP,
                    0,
                    220,
                    64,
                    80,
                    22,
                    ID_SWITCH_Z,
                    f,
                );

                i18n(
                    create_ctrl(hwnd, "STATIC", "Đặt dấu:", 0, 0, 24, 94, 85, 20, 0, f),
                    "Đặt dấu:",
                    "Tone style:",
                );
                let tone_new = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Mới (hoà, thuý)",
                        (BS_AUTORADIOBUTTON as u32) | WS_GROUP | WS_TABSTOP,
                        0,
                        112,
                        92,
                        140,
                        22,
                        ID_TONE_NEW,
                        f,
                    ),
                    "Mới (hoà, thuý)",
                    "Modern (hoà, thuý)",
                );
                let tone_old = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Cũ (hòa, thúy)",
                        (BS_AUTORADIOBUTTON as u32) | WS_TABSTOP,
                        0,
                        260,
                        92,
                        140,
                        22,
                        ID_TONE_OLD,
                        f,
                    ),
                    "Cũ (hòa, thúy)",
                    "Classic (hòa, thúy)",
                );

                // 2. Khung Tùy chọn khác
                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Tùy chọn khác",
                        BS_GROUPBOX as u32,
                        0,
                        12,
                        136,
                        560,
                        192,
                        0,
                        f,
                    ),
                    "Tùy chọn khác",
                    "Other options",
                );
                let chk_default_vi = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Bật tiếng Việt mặc định",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        156,
                        256,
                        22,
                        ID_CHK_DEFAULT_VI,
                        f,
                    ),
                    "Bật tiếng Việt mặc định",
                    "Enable Vietnamese by default",
                );
                let chk_free_typing = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Cho phép gõ tự do",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        180,
                        256,
                        22,
                        ID_CHK_FREE_TYPING,
                        f,
                    ),
                    "Cho phép gõ tự do",
                    "Allow free tone placement",
                );
                let chk_clipboard_unicode = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Luôn dùng clipboard cho Unicode",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        204,
                        256,
                        22,
                        ID_CHK_CLIPBOARD_UNICODE,
                        f,
                    ),
                    "Luôn dùng clipboard cho Unicode",
                    "Always use clipboard for Unicode",
                );
                let chk_w_as_u = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Phím w đứng riêng thành ư (Telex)",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        228,
                        256,
                        22,
                        ID_CHK_W_AS_U,
                        f,
                    ),
                    "Phím w đứng riêng thành ư (Telex)",
                    "Standalone w -> ư (Telex)",
                );
                let chk_brackets = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Phím [ / ] thành ơ / ư (Telex)",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        252,
                        256,
                        22,
                        ID_CHK_BRACKETS,
                        f,
                    ),
                    "Phím [ / ] thành ơ / ư (Telex)",
                    "Keys [ / ] -> ơ / ư (Telex)",
                );
                let chk_quick_telex = create_ctrl(
                    hwnd,
                    "BUTTON",
                    "Quick Telex (cc=ch, gg=gi...)",
                    (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                    0,
                    24,
                    276,
                    256,
                    22,
                    ID_CHK_QUICK_TELEX,
                    f,
                );
                let chk_coda_k = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Âm cuối k với sắc/nặng (Đắk Lắk)",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        300,
                        256,
                        22,
                        ID_CHK_CODA_K,
                        f,
                    ),
                    "Âm cuối k với sắc/nặng (Đắk Lắk)",
                    "Allow coda k with acute/dot (Đắk Lắk)",
                );

                let chk_spell = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Bật kiểm tra chính tả",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        292,
                        156,
                        270,
                        22,
                        ID_CHK_SPELL,
                        f,
                    ),
                    "Bật kiểm tra chính tả",
                    "Enable spell checking",
                );
                let chk_auto_restore = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Tự động khôi phục phím với từ sai",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        292,
                        180,
                        270,
                        22,
                        ID_CHK_AUTO_RESTORE,
                        f,
                    ),
                    "Tự động khôi phục phím với từ sai",
                    "Auto-restore keys for invalid words",
                );
                let chk_feedback_notify = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Hiện thông báo phản hồi",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        292,
                        204,
                        270,
                        22,
                        ID_CHK_FEEDBACK_NOTIFY,
                        f,
                    ),
                    "Hiện thông báo phản hồi",
                    "Show feedback notifications",
                );
                let chk_beep_switch = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Bật chuông khi chuyển ngôn ngữ",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        292,
                        228,
                        270,
                        22,
                        ID_CHK_BEEP_SWITCH,
                        f,
                    ),
                    "Bật chuông khi chuyển ngôn ngữ",
                    "Beep on language switch",
                );
                let chk_edit_prev = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Sửa từ trước bằng Backspace",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        292,
                        252,
                        270,
                        22,
                        ID_CHK_EDIT_PREV,
                        f,
                    ),
                    "Sửa từ trước bằng Backspace",
                    "Edit previous word on Backspace",
                );
                let chk_auto_cap = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Tự viết hoa đầu câu",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        292,
                        276,
                        270,
                        22,
                        ID_CHK_AUTO_CAP,
                        f,
                    ),
                    "Tự viết hoa đầu câu",
                    "Auto-capitalize first letter",
                );
                let chk_smart_caps = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Giữ chữ hoa 2 chữ cái đầu",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        292,
                        300,
                        270,
                        22,
                        ID_CHK_SMART_CAPS,
                        f,
                    ),
                    "Giữ chữ hoa 2 chữ cái đầu",
                    "Keep initial double capitals",
                );

                // 3. Khung Tùy chọn gõ tắt (trái) & Hệ thống (phải)
                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Tùy chọn gõ tắt",
                        BS_GROUPBOX as u32,
                        0,
                        12,
                        336,
                        274,
                        162,
                        0,
                        f,
                    ),
                    "Tùy chọn gõ tắt",
                    "Macro options",
                );
                let chk_macro_enabled = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Cho phép gõ tắt",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        356,
                        250,
                        22,
                        ID_CHK_MACRO_ENABLED,
                        f,
                    ),
                    "Cho phép gõ tắt",
                    "Enable macros",
                );
                let chk_macro_undo = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Backspace hoàn tác gõ tắt",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        380,
                        250,
                        22,
                        ID_CHK_MACRO_UNDO,
                        f,
                    ),
                    "Backspace hoàn tác gõ tắt",
                    "Backspace undoes macro",
                );
                let chk_macro_digits = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Cho phép chữ số trong gõ tắt",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        404,
                        250,
                        22,
                        ID_CHK_MACRO_DIGITS,
                        f,
                    ),
                    "Cho phép chữ số trong gõ tắt",
                    "Allow digits in macros",
                );
                let chk_macro_off = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Bung gõ tắt khi tắt tiếng Việt",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        24,
                        428,
                        250,
                        22,
                        ID_CHK_MACRO_OFF,
                        f,
                    ),
                    "Bung gõ tắt khi tắt tiếng Việt",
                    "Expand macros in English mode",
                );
                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Bảng gõ tắt (macros.txt)...",
                        (BS_PUSHBUTTON as u32) | WS_TABSTOP,
                        0,
                        24,
                        458,
                        172,
                        26,
                        ID_BTN_OPEN_MACROS,
                        f,
                    ),
                    "Bảng gõ tắt (macros.txt)...",
                    "Macro table (macros.txt)...",
                );
                let lbl_macro_count =
                    create_ctrl(hwnd, "STATIC", "(0 quy tắc)", 0, 0, 202, 463, 76, 18, 0, f);

                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Hệ thống && Ứng dụng",
                        BS_GROUPBOX as u32,
                        0,
                        298,
                        336,
                        274,
                        162,
                        0,
                        f,
                    ),
                    "Hệ thống && Ứng dụng",
                    "System && Apps",
                );
                let chk_show_on_start = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Bật hội thoại này khi khởi động",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        310,
                        356,
                        252,
                        22,
                        ID_CHK_SHOW_ON_START,
                        f,
                    ),
                    "Bật hội thoại này khi khởi động",
                    "Show this dialog on startup",
                );
                let chk_autostart = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Khởi động cùng Windows",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        310,
                        380,
                        252,
                        22,
                        ID_CHK_AUTOSTART,
                        f,
                    ),
                    "Khởi động cùng Windows",
                    "Start with Windows",
                );
                let chk_per_app_mode = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Bật/Tắt theo từng ứng dụng",
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        0,
                        310,
                        404,
                        252,
                        22,
                        ID_CHK_PER_APP_MODE,
                        f,
                    ),
                    "Bật/Tắt theo từng ứng dụng",
                    "Switch mode per application",
                );
                let chk_viet_ui = create_ctrl(
                    hwnd,
                    "BUTTON",
                    "Vietnamese interface",
                    (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                    0,
                    310,
                    428,
                    252,
                    22,
                    ID_CHK_VIET_UI,
                    f,
                );
                let btn_toggle_mode = create_ctrl(
                    hwnd,
                    "BUTTON",
                    "Đang bật: Tiếng Việt",
                    (BS_PUSHBUTTON as u32) | WS_TABSTOP,
                    0,
                    310,
                    458,
                    150,
                    26,
                    ID_BTN_TOGGLE_MODE,
                    f,
                );
                let lbl_ipc_status =
                    create_ctrl(hwnd, "STATIC", "Hook: Portable", 0, 0, 466, 463, 98, 18, 0, f);

                // 4. Thanh nút dưới cùng
                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Thông tin",
                        (BS_PUSHBUTTON as u32) | WS_TABSTOP,
                        0,
                        12,
                        508,
                        82,
                        28,
                        ID_BTN_INFO,
                        f,
                    ),
                    "Thông tin",
                    "About",
                );
                if state.bridge.is_some() {
                    i18n(
                        create_ctrl(
                            hwnd,
                            "BUTTON",
                            "Kết thúc",
                            (BS_PUSHBUTTON as u32) | WS_TABSTOP,
                            0,
                            102,
                            508,
                            82,
                            28,
                            ID_BTN_EXIT,
                            f,
                        ),
                        "Kết thúc",
                        "Quit",
                    );
                }
                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Mặc định",
                        (BS_PUSHBUTTON as u32) | WS_TABSTOP,
                        0,
                        298,
                        508,
                        82,
                        28,
                        ID_BTN_DEFAULTS,
                        f,
                    ),
                    "Mặc định",
                    "Defaults",
                );
                let btn_revert = i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Hoàn tác",
                        (BS_PUSHBUTTON as u32) | WS_TABSTOP,
                        0,
                        388,
                        508,
                        82,
                        28,
                        ID_BTN_REVERT,
                        f,
                    ),
                    "Hoàn tác",
                    "Revert",
                );
                i18n(
                    create_ctrl(
                        hwnd,
                        "BUTTON",
                        "Lưu && Đóng",
                        (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP,
                        0,
                        478,
                        508,
                        94,
                        28,
                        ID_BTN_SAVE_CLOSE,
                        f,
                    ),
                    "Lưu && Đóng",
                    "Save && Close",
                );

                state.controls = Some(Controls {
                    charset,
                    input_method,
                    switch_comma,
                    switch_z,
                    tone_new,
                    tone_old,
                    chk_default_vi,
                    chk_free_typing,
                    chk_clipboard_unicode,
                    chk_spell,
                    chk_auto_restore,
                    chk_feedback_notify,
                    chk_beep_switch,
                    chk_w_as_u,
                    chk_brackets,
                    chk_quick_telex,
                    chk_coda_k,
                    chk_edit_prev,
                    chk_auto_cap,
                    chk_smart_caps,
                    chk_macro_enabled,
                    chk_macro_undo,
                    chk_macro_digits,
                    chk_macro_off,
                    lbl_macro_count,
                    chk_show_on_start,
                    chk_autostart,
                    chk_per_app_mode,
                    chk_viet_ui,
                    btn_toggle_mode,
                    lbl_ipc_status,
                    btn_revert,
                });
                state.ctrl_layouts =
                    PENDING_LAYOUTS.with(|layouts| std::mem::take(&mut *layouts.borrow_mut()));
            }
        }

        unsafe extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            if ptr.is_null() {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
            let state = unsafe { &mut *ptr };

            match msg {
                WM_BRIDGE_WAKE => {
                    if let Some(ref bridge) = state.bridge {
                        if bridge.exit_requested.swap(false, Ordering::AcqRel) {
                            state.allow_close = true;
                            unsafe { DestroyWindow(hwnd) };
                            return 0;
                        }
                        if bridge.refresh_requested.swap(false, Ordering::AcqRel) {
                            state.vm = SettingsViewModel::load(state.vm.config_path.clone());
                            sync_ui_from_vm(state);
                        }
                        if bridge.show_requested.swap(false, Ordering::AcqRel) {
                            let dpi = unsafe { GetDpiForWindow(hwnd).max(96) };
                            if dpi != current_window_dpi() {
                                unsafe { apply_dpi_layout(hwnd, state, dpi, None) };
                            }
                            unsafe {
                                ShowWindow(hwnd, SW_SHOWNORMAL);
                                SetForegroundWindow(hwnd);
                            }
                        }
                        update_bridge_labels(state);
                    }
                    0
                }
                WM_COMMAND => {
                    let ctrl_id = (wparam & 0xFFFF) as i32;
                    match ctrl_id {
                        ID_BTN_OPEN_MACROS => {
                            if let Ok(path) = state.vm.ensure_macros_file() {
                                let op = to_wide("open");
                                let wpath = to_wide(&path.to_string_lossy());
                                unsafe {
                                    ShellExecuteW(
                                        hwnd,
                                        op.as_ptr(),
                                        wpath.as_ptr(),
                                        null(),
                                        null(),
                                        SW_SHOWNORMAL,
                                    );
                                }
                                sync_ui_from_vm(state);
                            }
                        }
                        ID_CHK_AUTOSTART => {
                            if let Some(ref c) = state.controls {
                                set_autostart(unsafe { is_checked(c.chk_autostart) });
                            }
                        }
                        ID_BTN_TOGGLE_MODE => {
                            if let Some(ref cb) = state.on_toggle {
                                cb();
                            }
                            update_bridge_labels(state);
                        }
                        ID_BTN_INFO => {
                            let msg_txt = to_wide(&format!(
                                "VNType {} — Bộ gõ tiếng Việt\n\nCấu hình: {}\nGõ tắt: {}",
                                env!("CARGO_PKG_VERSION"),
                                state.vm.config_path.display(),
                                state.vm.macros_path().display()
                            ));
                            let title = to_wide("Thông tin VNType");
                            unsafe {
                                MessageBoxW(
                                    hwnd,
                                    msg_txt.as_ptr(),
                                    title.as_ptr(),
                                    MB_OK | MB_ICONINFORMATION,
                                );
                            }
                        }
                        ID_BTN_DEFAULTS => {
                            state.vm.reset_to_defaults();
                            sync_ui_from_vm(state);
                        }
                        ID_BTN_REVERT => {
                            state.vm.reset_draft_to_saved();
                            sync_ui_from_vm(state);
                        }
                        ID_BTN_SAVE_CLOSE => {
                            sync_vm_from_ui(state);
                            match state.vm.apply_and_save() {
                                Ok(()) => {
                                    if let Some(ref routing) = state.routing {
                                        routing.notify_config_reloaded();
                                    }
                                    if let Some(ref cb) = state.on_config_changed {
                                        cb();
                                    }
                                    if state.bridge.is_some() {
                                        unsafe { ShowWindow(hwnd, SW_HIDE) };
                                    } else {
                                        state.allow_close = true;
                                        unsafe { DestroyWindow(hwnd) };
                                    }
                                }
                                Err(err) => {
                                    let msg_txt = to_wide(&format!("Không thể lưu cấu hình: {err}"));
                                    let title = to_wide("Lỗi");
                                    unsafe {
                                        MessageBoxW(
                                            hwnd,
                                            msg_txt.as_ptr(),
                                            title.as_ptr(),
                                            MB_OK | MB_ICONERROR,
                                        );
                                    }
                                }
                            }
                        }
                        ID_BTN_EXIT => {
                            if let Some(ref on_quit) = state.on_quit {
                                on_quit();
                            }
                            state.allow_close = true;
                            unsafe { DestroyWindow(hwnd) };
                        }
                        ID_CHK_VIET_UI => {
                            sync_vm_from_ui(state);
                            apply_interface_language(state);
                        }
                        _ => {
                            sync_vm_from_ui(state);
                        }
                    }
                    0
                }
                WM_DPICHANGED => {
                    let new_dpi = ((wparam & 0xFFFF) as u32).max(96);
                    let suggested =
                        if lparam != 0 { Some(unsafe { &*(lparam as *const RECT) }) } else { None };
                    unsafe { apply_dpi_layout(hwnd, state, new_dpi, suggested) };
                    0
                }
                WM_CLOSE => {
                    if state.bridge.is_some() && !state.allow_close {
                        unsafe { ShowWindow(hwnd, SW_HIDE) };
                        0
                    } else {
                        unsafe { DestroyWindow(hwnd) };
                        0
                    }
                }
                WM_DESTROY => {
                    if let Some(ref bridge) = state.bridge {
                        bridge.hwnd.store(0, Ordering::Release);
                    }
                    unsafe { PostQuitMessage(0) };
                    0
                }
                _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
        }

        pub fn run_window(
            vm: SettingsViewModel,
            bridge: Option<Arc<super::HookGuiBridge>>,
            on_toggle: Option<Box<dyn Fn() + Send>>,
            on_config_changed: Option<Box<dyn Fn() + Send>>,
            on_quit: Option<Box<dyn Fn() + Send>>,
        ) {
            unsafe {
                let icc = INITCOMMONCONTROLSEX {
                    dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_STANDARD_CLASSES,
                };
                InitCommonControlsEx(&icc);

                let hinst = GetModuleHandleW(null());
                let icon_big = LoadImageW(
                    hinst,
                    1usize as *const u16,
                    IMAGE_ICON,
                    GetSystemMetrics(SM_CXICON),
                    GetSystemMetrics(SM_CYICON),
                    LR_DEFAULTCOLOR,
                ) as HICON;
                let icon_sm = LoadImageW(
                    hinst,
                    1usize as *const u16,
                    IMAGE_ICON,
                    GetSystemMetrics(SM_CXSMICON),
                    GetSystemMetrics(SM_CYSMICON),
                    LR_DEFAULTCOLOR,
                ) as HICON;
                let class_name = to_wide("VNTypeSettingsWin32");
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: 0,
                    lpfnWndProc: Some(wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinst,
                    hIcon: icon_big,
                    hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                    hbrBackground: (COLOR_BTNFACE + 1) as _,
                    lpszMenuName: null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: icon_sm,
                };
                RegisterClassExW(&wc);

                let title = to_wide(concat!("VNType ", env!("CARGO_PKG_VERSION")));
                let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

                let hwnd = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    title.as_ptr(),
                    style,
                    120,
                    120,
                    600,
                    528,
                    null_mut(),
                    null_mut(),
                    hinst,
                    null(),
                );
                if hwnd.is_null() {
                    return;
                }

                let dpi = GetDpiForWindow(hwnd).max(96);
                set_window_dpi(dpi);
                let mut rect = RECT { left: 0, top: 0, right: scale_px(584), bottom: scale_px(548) };
                AdjustWindowRectExForDpi(&mut rect, style, 0, 0, dpi);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );

                let font = create_ui_font();
                let routing = SharedRoutingMapping::open_or_create().ok();
                let start_hidden = bridge.is_some() && !vm.draft.show_dialog_on_startup;

                let mut state = Box::new(WindowState {
                    vm,
                    caps: PlatformCapabilities::windows(),
                    routing,
                    bridge: bridge.clone(),
                    on_toggle,
                    on_config_changed,
                    on_quit,
                    allow_close: bridge.is_none(),
                    font,
                    controls: None,
                    ctrl_layouts: Vec::new(),
                    i18n_ctrls: Vec::new(),
                });

                build_controls(hwnd, &mut state);
                sync_ui_from_vm(&state);

                let raw_state = Box::into_raw(state);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw_state as isize);

                if let Some(ref b) = bridge {
                    b.hwnd.store(hwnd as isize, Ordering::Release);
                }

                if !start_hidden {
                    ShowWindow(hwnd, SW_SHOWNORMAL);
                }

                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                let boxed = Box::from_raw(raw_state);
                if !boxed.font.is_null() {
                    DeleteObject(boxed.font as _);
                }
            }
        }
    }

    /// Khởi chạy cửa sổ giao diện cấu hình VNType ở chế độ độc lập.
    pub fn run_settings_gui(vm: SettingsViewModel) {
        #[cfg(windows)]
        win32_impl::run_window(vm, None, None, None, None);
        #[cfg(not(windows))]
        let _ = vm;
    }

    /// Khởi chạy cửa sổ giao diện cấu hình nhúng trên luồng chính cùng với `vntype-hook`.
    pub fn run_embedded_gui(
        config_path: PathBuf,
        bridge: Arc<HookGuiBridge>,
        on_toggle: Box<dyn Fn() + Send>,
        on_config_changed: Box<dyn Fn() + Send>,
        on_quit: Box<dyn Fn() + Send>,
    ) {
        #[cfg(windows)]
        {
            let vm = SettingsViewModel::load(config_path);
            win32_impl::run_window(
                vm,
                Some(bridge),
                Some(on_toggle),
                Some(on_config_changed),
                Some(on_quit),
            );
        }
        #[cfg(not(windows))]
        let _ = (config_path, bridge, on_toggle, on_config_changed, on_quit);
    }
}
