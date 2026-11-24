//! Bộ điều khiển Hook bàn phím mức thấp (`WH_KEYBOARD_LL`) + `SendInput` dự phòng trên Windows.
//!
//! Bộ điều khiển này sử dụng [`vntype_adapter::AdapterController`] ở chế độ `OutputMode::Replace`
//! với `allow_unverified_replace = true`, và tự động nhường quyền (`bypass`) khi vùng nhớ dùng chung
//! [`vntype_win_ipc::SharedRoutingBlock`] báo rằng TSF đang kiểm soát tiến trình hiện hành.

use vntype_adapter::{AdapterController, Effect};
use vntype_engine::{Config, KeyEvent, OutputMode};
use vntype_win_ipc::SharedRoutingBlock;

/// Lệnh tổng hợp bàn phím (`SendInput`) phát ra từ trình nền Hook trên Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinInputBatch {
    /// Gửi `count` lần phím Backspace (`VK_BACK`) để xoá các ký tự cũ.
    SendBackspaces(u32),
    /// Gửi chuỗi ký tự Unicode qua cờ `KEYEVENTF_UNICODE`.
    SendUnicodeText(String),
}

/// Bộ xử lý sự kiện phím cho trình nền Windows Hook.
pub struct WindowsHookProcessor {
    controller: AdapterController,
}

impl WindowsHookProcessor {
    /// Khởi tạo bộ xử lý Windows Hook với cấu hình `config`.
    pub fn new(config: Config) -> Self {
        let mut controller = AdapterController::new(config, OutputMode::Replace);
        controller.set_allow_unverified_replace(true);
        WindowsHookProcessor { controller }
    }

    /// Xử lý một sự kiện phím từ `WH_KEYBOARD_LL`.
    ///
    /// Nếu `routing` cho biết tiến trình `foreground_pid` đang được `vntype-tsf` xử lý ở chế độ Preedit,
    /// hàm lập tức trả về `(false, Vec::new())` để nhường hoàn toàn cho TSF.
    pub fn process_hook_key(
        &mut self,
        routing: &SharedRoutingBlock,
        foreground_pid: u32,
        event: KeyEvent,
        vk_code: u32,
    ) -> (bool, Vec<WinInputBatch>) {
        if routing.should_hook_bypass(foreground_pid) {
            self.controller.reset();
            return (false, Vec::new());
        }

        let effects = self.controller.process_key(event, vk_code);
        let mut batches = Vec::new();
        let mut forward_original = false;

        for eff in effects {
            match eff {
                Effect::CommitText { text } => {
                    if !text.is_empty() {
                        batches.push(WinInputBatch::SendUnicodeText(text));
                    }
                }
                Effect::ReplaceText { delete_utf16, insert, .. } => {
                    if delete_utf16 > 0 {
                        batches.push(WinInputBatch::SendBackspaces(delete_utf16));
                    }
                    if !insert.is_empty() {
                        batches.push(WinInputBatch::SendUnicodeText(insert));
                    }
                }
                Effect::ForwardKey { .. } => {
                    forward_original = true;
                }
                _ => {}
            }
        }

        let consume_physical_key = !batches.is_empty() && !forward_original;
        (consume_physical_key, batches)
    }

    /// Bật/tắt chế độ gõ tiếng Việt.
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.controller.is_enabled() != enabled {
            let _ = self.controller.toggle_vietnamese();
        }
    }

    /// Trả về trạng thái bật/tắt tiếng Việt hiện tại.
    pub fn is_enabled(&self) -> bool {
        self.controller.is_enabled()
    }

    /// Cập nhật cấu hình mới cho bộ điều khiển Hook.
    pub fn reload_config(&mut self, config: Config) {
        let mut controller = AdapterController::new(config, OutputMode::Replace);
        controller.set_allow_unverified_replace(true);
        self.controller = controller;
    }

    /// Cập nhật tên ứng dụng đang có tiêu điểm để áp dụng per-app rules.
    pub fn notify_focus(&mut self, app_name: Option<&str>) {
        let _ = self.controller.focus_changed(app_name);
    }
}

#[cfg(windows)]
pub mod win_hook {
    use std::cell::RefCell;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows_sys::Win32::Foundation::{CloseHandle, HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetKeyboardState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, SendInput, ToUnicode, VK_BACK, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_V,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
        HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PostQuitMessage, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    use vntype_config::{
        SwitchKey, UnicodeForm, default_config_path, load_config_or_default, save_config_atomic,
    };
    use vntype_engine::{Config, Key, KeyEvent};
    use vntype_settings::gui::win_gui::HookGuiBridge;
    use vntype_win_ipc::win_shm::SharedRoutingMapping;

    use crate::tray::{TrayMenuAction, TrayStatus, win_tray::WinTrayIcon};

    use super::{WinInputBatch, WindowsHookProcessor};

    const VNTYPE_INJECTED_TAG: usize = 0x564E_5450;
    const VK_OEM_COMMA: u32 = 0xBC;
    const VK_Z: u32 = 0x5A;

    fn is_ctrl_vk(vk: u32) -> bool {
        matches!(vk, 0x11 | 0xA2 | 0xA3)
    }

    fn is_shift_vk(vk: u32) -> bool {
        matches!(vk, 0x10 | 0xA0 | 0xA1)
    }

    thread_local! {
        static CTRL_DOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static SHIFT_DOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static ALT_DOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static CTRL_SHIFT_CHORD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static OTHER_KEY_PRESSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    struct HookRuntime {
        processor: WindowsHookProcessor,
        config: Config,
        routing: SharedRoutingMapping,
        tray: Option<WinTrayIcon>,
        bridge: Arc<HookGuiBridge>,
        last_config_gen: u64,
        last_app: String,
    }

    thread_local! {
        static HOOK_RUNTIME: RefCell<Option<HookRuntime>> = const { RefCell::new(None) };
        static HOOK_HANDLE: RefCell<HHOOK> = const { RefCell::new(core::ptr::null_mut()) };
        static WIN_KEY_DOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    fn method_label(method: &str) -> String {
        match method {
            "vni" => "VNI".into(),
            "simple_telex" => "Simple Telex".into(),
            "viqr" => "VIQR".into(),
            _ => "Telex".into(),
        }
    }

    fn build_tray_status(cfg: &Config, vietnamese_enabled: bool) -> TrayStatus {
        let charset = if cfg.charset == "unicode" && cfg.unicode_form == UnicodeForm::Nfd {
            "unicode_nfd".into()
        } else {
            cfg.charset.clone()
        };
        TrayStatus {
            vietnamese_enabled,
            input_method_label: method_label(&cfg.input_method),
            charset,
            switch_key: cfg.switch_key,
        }
    }

    fn paste_text_via_clipboard(text: &str) -> bool {
        const CF_UNICODETEXT: u32 = 13;
        let wide: Vec<u16> = text.encode_utf16().chain(core::iter::once(0)).collect();
        let byte_len = wide.len() * core::mem::size_of::<u16>();
        unsafe {
            if OpenClipboard(core::ptr::null_mut()) == 0 {
                return false;
            }
            EmptyClipboard();
            let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
            if hmem.is_null() {
                CloseClipboard();
                return false;
            }
            let ptr = GlobalLock(hmem) as *mut u16;
            if ptr.is_null() {
                CloseClipboard();
                return false;
            }
            core::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            GlobalUnlock(hmem);
            let set_ok = !SetClipboardData(CF_UNICODETEXT, hmem).is_null();
            CloseClipboard();
            if !set_ok {
                return false;
            }

            let inputs = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_CONTROL,
                            wScan: 0,
                            dwFlags: 0,
                            time: 0,
                            dwExtraInfo: VNTYPE_INJECTED_TAG,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_V,
                            wScan: 0,
                            dwFlags: 0,
                            time: 0,
                            dwExtraInfo: VNTYPE_INJECTED_TAG,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_V,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: VNTYPE_INJECTED_TAG,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_CONTROL,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: VNTYPE_INJECTED_TAG,
                        },
                    },
                },
            ];
            SendInput(inputs.len() as u32, inputs.as_ptr(), core::mem::size_of::<INPUT>() as i32);
            true
        }
    }

    fn flush_inputs(inputs: &mut Vec<INPUT>) {
        if !inputs.is_empty() {
            unsafe {
                SendInput(inputs.len() as u32, inputs.as_ptr(), core::mem::size_of::<INPUT>() as i32);
            }
            inputs.clear();
        }
    }

    /// Gửi các batch backspace / Unicode tới ứng dụng đích qua `SendInput` (hoặc Clipboard nếu bật).
    pub fn send_input_batches(batches: &[WinInputBatch], use_clipboard: bool) {
        let mut inputs: Vec<INPUT> = Vec::new();
        for batch in batches {
            match batch {
                WinInputBatch::SendBackspaces(count) => {
                    for _ in 0..*count {
                        inputs.push(INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_BACK,
                                    wScan: 0,
                                    dwFlags: 0,
                                    time: 0,
                                    dwExtraInfo: VNTYPE_INJECTED_TAG,
                                },
                            },
                        });
                        inputs.push(INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_BACK,
                                    wScan: 0,
                                    dwFlags: KEYEVENTF_KEYUP,
                                    time: 0,
                                    dwExtraInfo: VNTYPE_INJECTED_TAG,
                                },
                            },
                        });
                    }
                }
                WinInputBatch::SendUnicodeText(text) => {
                    if use_clipboard {
                        flush_inputs(&mut inputs);
                        if paste_text_via_clipboard(text) {
                            continue;
                        }
                    }
                    for unit in text.encode_utf16() {
                        inputs.push(INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: 0,
                                    wScan: unit,
                                    dwFlags: KEYEVENTF_UNICODE,
                                    time: 0,
                                    dwExtraInfo: VNTYPE_INJECTED_TAG,
                                },
                            },
                        });
                        inputs.push(INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: 0,
                                    wScan: unit,
                                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                    time: 0,
                                    dwExtraInfo: VNTYPE_INJECTED_TAG,
                                },
                            },
                        });
                    }
                }
            }
        }
        flush_inputs(&mut inputs);
    }

    /// Lấy `(pid, exe_stem)` của cửa sổ đang có tiêu điểm.
    pub fn foreground_process_info() -> (u32, String) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return (0, String::new());
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return (0, String::new());
            }
            let proc_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if proc_handle.is_null() {
                return (pid, String::new());
            }
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(proc_handle, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(proc_handle);
            if ok == 0 || len == 0 {
                return (pid, String::new());
            }
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            let stem = Path::new(&full).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            (pid, stem)
        }
    }

    fn current_modifiers() -> (bool, bool, bool) {
        unsafe {
            let ctrl = (GetAsyncKeyState(VK_CONTROL as i32) as u16) & 0x8000 != 0;
            let alt = (GetAsyncKeyState(VK_MENU as i32) as u16) & 0x8000 != 0;
            let async_win = ((GetAsyncKeyState(VK_LWIN as i32) as u16) & 0x8000 != 0)
                || ((GetAsyncKeyState(VK_RWIN as i32) as u16) & 0x8000 != 0);
            let meta = async_win || WIN_KEY_DOWN.with(|c| c.get());
            (ctrl, alt, meta)
        }
    }

    fn translate_vk_to_key_event(vk: u32, scan_code: u32) -> KeyEvent {
        let (ctrl, alt, meta) = current_modifiers();
        let key = match vk as u16 {
            0x08 => Key::Backspace,
            0x0D => Key::Enter,
            0x09 => Key::Tab,
            0x1B => Key::Escape,
            0x25 => Key::Left,
            0x26 => Key::Up,
            0x27 => Key::Right,
            0x28 => Key::Down,
            0x24 => Key::Home,
            0x23 => Key::End,
            0x2E => Key::Delete,
            0x21 => Key::PageUp,
            0x22 => Key::PageDown,
            0x10..=0x14 | 0x5B..=0x5C | 0xA0..=0xA5 => Key::Modifier,
            _ => unsafe {
                let mut kb_state = [0u8; 256];
                GetKeyboardState(kb_state.as_mut_ptr());
                let mut out = [0u16; 4];
                let n = ToUnicode(vk, scan_code, kb_state.as_ptr(), out.as_mut_ptr(), out.len() as i32, 0);
                if n == 1 {
                    match char::from_u32(out[0] as u32) {
                        Some(c) if !c.is_control() => Key::Char(c),
                        _ => Key::Other,
                    }
                } else {
                    Key::Other
                }
            },
        };
        KeyEvent { key, ctrl, alt, meta }
    }

    fn notify_switch_feedback(rt: &HookRuntime, enabled: bool) {
        if rt.config.beep_on_switch {
            unsafe {
                MessageBeep(0x0000_0040);
            }
        }
        if rt.config.show_feedback_notification {
            if let Some(tray) = &rt.tray {
                let msg = if rt.config.vietnamese_interface {
                    if enabled {
                        "Đã bật chế độ gõ Tiếng Việt"
                    } else {
                        "Đã tắt chế độ gõ Tiếng Việt (Tiếng Anh)"
                    }
                } else if enabled {
                    "Vietnamese typing: ON"
                } else {
                    "Vietnamese typing: OFF (English)"
                };
                tray.show_balloon("VNType", msg);
            }
        }
    }

    fn sync_ui_and_actions(rt: &mut HookRuntime) {
        let cfg_gen = rt.routing.config_generation.load(Ordering::Acquire);
        if cfg_gen != rt.last_config_gen {
            rt.config = load_config_or_default(default_config_path());
            rt.processor.reload_config(rt.config.clone());
            rt.processor.set_enabled(rt.config.default_vietnamese);
            rt.last_config_gen = cfg_gen;
            rt.bridge.set_vietnamese_enabled(rt.processor.is_enabled());
            rt.bridge.request_refresh();
            if let Some(tray) = &rt.tray {
                tray.register_switch_hotkey(rt.config.switch_key);
                tray.update(build_tray_status(&rt.config, rt.processor.is_enabled()));
            }
        }

        while let Some(act) = rt.tray.as_ref().and_then(|t| t.take_pending_action()) {
            match act {
                TrayMenuAction::ToggleVietnamese => {
                    let next = !rt.processor.is_enabled();
                    rt.processor.set_enabled(next);
                    rt.config.default_vietnamese = next;
                    let _ = save_config_atomic(default_config_path(), &rt.config);
                    rt.bridge.set_vietnamese_enabled(next);
                    rt.bridge.request_refresh();
                    notify_switch_feedback(rt, next);
                }
                TrayMenuAction::SelectTelex => {
                    rt.config.input_method = "telex".into();
                    let _ = save_config_atomic(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                    rt.bridge.request_refresh();
                }
                TrayMenuAction::SelectVni => {
                    rt.config.input_method = "vni".into();
                    let _ = save_config_atomic(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                    rt.bridge.request_refresh();
                }
                TrayMenuAction::SelectSimpleTelex => {
                    rt.config.input_method = "simple_telex".into();
                    let _ = save_config_atomic(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                    rt.bridge.request_refresh();
                }
                TrayMenuAction::SelectViqr => {
                    rt.config.input_method = "viqr".into();
                    let _ = save_config_atomic(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                    rt.bridge.request_refresh();
                }
                TrayMenuAction::SelectCharset(cs) => {
                    rt.config.charset = cs.into();
                    rt.config.unicode_form =
                        if cs == "unicode_nfd" { UnicodeForm::Nfd } else { UnicodeForm::Nfc };
                    let _ = save_config_atomic(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                    rt.bridge.request_refresh();
                }
                TrayMenuAction::OpenSettingsWindow => {
                    rt.bridge.request_show();
                }
                TrayMenuAction::ExitApplication => {
                    rt.bridge.request_exit();
                    unsafe {
                        PostQuitMessage(0);
                    }
                }
            }
            if let Some(tray) = &rt.tray {
                tray.update(build_tray_status(&rt.config, rt.processor.is_enabled()));
            }
        }
    }

    unsafe extern "system" fn ll_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let next_hook = HOOK_HANDLE.with(|h| *h.borrow());
        if code != HC_ACTION as i32 {
            return unsafe { CallNextHookEx(next_hook, code, wparam, lparam) };
        }

        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let msg = wparam as u32;
        if (kb.flags & LLKHF_INJECTED) == 0 && (kb.vkCode == VK_LWIN as u32 || kb.vkCode == VK_RWIN as u32)
        {
            let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            if is_down {
                WIN_KEY_DOWN.with(|c| c.set(true));
            } else if is_up {
                WIN_KEY_DOWN.with(|c| c.set(false));
            }
        }

        if (kb.flags & LLKHF_INJECTED) != 0 || kb.dwExtraInfo == VNTYPE_INJECTED_TAG {
            return unsafe { CallNextHookEx(next_hook, code, wparam, lparam) };
        }

        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        if matches!(kb.vkCode, 0x12 | 0xA4 | 0xA5) {
            ALT_DOWN.with(|c| c.set(is_down));
        }
        if is_down {
            if is_ctrl_vk(kb.vkCode) {
                CTRL_DOWN.with(|c| c.set(true));
            }
            if is_shift_vk(kb.vkCode) {
                SHIFT_DOWN.with(|c| c.set(true));
            }
            if is_ctrl_vk(kb.vkCode) || is_shift_vk(kb.vkCode) {
                let ctrl_down = CTRL_DOWN.with(|c| c.get());
                let shift_down = SHIFT_DOWN.with(|c| c.get());
                let alt_down = ALT_DOWN.with(|c| c.get());
                let win_down = WIN_KEY_DOWN.with(|c| c.get());
                if ctrl_down && shift_down && !alt_down && !win_down {
                    CTRL_SHIFT_CHORD.with(|c| c.set(true));
                    OTHER_KEY_PRESSED.with(|c| c.set(false));
                }
            } else {
                OTHER_KEY_PRESSED.with(|c| c.set(true));
                CTRL_SHIFT_CHORD.with(|c| c.set(false));
            }
        } else if is_up {
            if is_ctrl_vk(kb.vkCode) {
                CTRL_DOWN.with(|c| c.set(false));
            }
            if is_shift_vk(kb.vkCode) {
                SHIFT_DOWN.with(|c| c.set(false));
            }
            if is_ctrl_vk(kb.vkCode) || is_shift_vk(kb.vkCode) {
                let triggered =
                    CTRL_SHIFT_CHORD.with(|c| c.replace(false)) && !OTHER_KEY_PRESSED.with(|c| c.get());
                if triggered {
                    HOOK_RUNTIME.with(|cell| {
                        let mut borrow = cell.borrow_mut();
                        let Some(rt) = borrow.as_mut() else {
                            return;
                        };
                        sync_ui_and_actions(rt);
                        if rt.config.switch_key == SwitchKey::CtrlShift {
                            let next = !rt.processor.is_enabled();
                            rt.processor.set_enabled(next);
                            rt.config.default_vietnamese = next;
                            let _ = save_config_atomic(default_config_path(), &rt.config);
                            rt.bridge.set_vietnamese_enabled(next);
                            rt.bridge.request_refresh();
                            notify_switch_feedback(rt, next);
                            if let Some(tray) = &rt.tray {
                                tray.update(build_tray_status(&rt.config, next));
                            }
                        }
                    });
                }
                return unsafe { CallNextHookEx(next_hook, code, wparam, lparam) };
            }
        }

        if !is_down {
            return unsafe { CallNextHookEx(next_hook, code, wparam, lparam) };
        }

        let event = translate_vk_to_key_event(kb.vkCode, kb.scanCode);
        let consume = HOOK_RUNTIME.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let Some(rt) = borrow.as_mut() else {
                return false;
            };
            sync_ui_and_actions(rt);

            // Nếu `RegisterHotKey` chưa đăng ký được, dùng hook dự phòng để bắt phím chuyển.
            let is_switch_hotkey = !WinTrayIcon::is_hotkey_registered()
                && match rt.config.switch_key {
                    SwitchKey::CtrlShift => false,
                    SwitchKey::AltComma => {
                        kb.vkCode == VK_OEM_COMMA && event.alt && !event.ctrl && !event.meta
                    }
                    SwitchKey::AltZ => kb.vkCode == VK_Z && event.alt && !event.ctrl && !event.meta,
                };
            if is_switch_hotkey {
                let next = !rt.processor.is_enabled();
                rt.processor.set_enabled(next);
                rt.config.default_vietnamese = next;
                let _ = save_config_atomic(default_config_path(), &rt.config);
                rt.bridge.set_vietnamese_enabled(next);
                rt.bridge.request_refresh();
                notify_switch_feedback(rt, next);
                if let Some(tray) = &rt.tray {
                    tray.update(build_tray_status(&rt.config, next));
                }
                return true;
            }

            let (pid, app) = foreground_process_info();
            if app != rt.last_app {
                rt.last_app = app.clone();
                let opt_app = if app.is_empty() { None } else { Some(app.as_str()) };
                rt.processor.notify_focus(opt_app);
            }

            let (consumed, batches) = rt.processor.process_hook_key(&rt.routing, pid, event, kb.vkCode);
            if !batches.is_empty() {
                send_input_batches(&batches, rt.config.always_use_clipboard_unicode);
            }
            consumed
        });

        if consume { 1 } else { unsafe { CallNextHookEx(next_hook, code, wparam, lparam) } }
    }

    /// Khởi chạy vòng lặp thông điệp Win32 cho `WH_KEYBOARD_LL` và Tray.
    pub fn run_hook_event_loop(
        initial_config: Config,
        bridge: Arc<HookGuiBridge>,
        tray_hwnd_out: Arc<AtomicIsize>,
    ) -> Result<(), u32> {
        let routing = SharedRoutingMapping::open_or_create()?;
        let last_config_gen = routing.config_generation.load(Ordering::Acquire);
        let processor = WindowsHookProcessor::new(initial_config.clone());
        let initial_vn = processor.is_enabled();
        let tray = WinTrayIcon::new(build_tray_status(&initial_config, initial_vn));
        if let Some(t) = &tray {
            tray_hwnd_out.store(t.hwnd_raw(), Ordering::Release);
            t.register_switch_hotkey(initial_config.switch_key);
        }
        bridge.set_vietnamese_enabled(initial_vn);

        HOOK_RUNTIME.with(|cell| {
            *cell.borrow_mut() = Some(HookRuntime {
                processor,
                config: initial_config,
                routing,
                tray,
                bridge,
                last_config_gen,
                last_app: String::new(),
            });
        });

        unsafe {
            let hmodule: HINSTANCE = GetModuleHandleW(core::ptr::null());
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), hmodule, 0);
            if hook.is_null() {
                return Err(windows_sys::Win32::Foundation::GetLastError());
            }
            HOOK_HANDLE.with(|h| *h.borrow_mut() = hook);

            let mut msg: MSG = core::mem::zeroed();
            while GetMessageW(&mut msg, core::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
                HOOK_RUNTIME.with(|cell| {
                    if let Some(rt) = cell.borrow_mut().as_mut() {
                        sync_ui_and_actions(rt);
                    }
                });
            }

            UnhookWindowsHookEx(hook);
            HOOK_HANDLE.with(|h| *h.borrow_mut() = core::ptr::null_mut());
            HOOK_RUNTIME.with(|cell| *cell.borrow_mut() = None);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_bypasses_when_tsf_is_active_for_same_pid() {
        let routing = SharedRoutingBlock::new();
        let mut hook = WindowsHookProcessor::new(Config::default());

        // Khi TSF không hoạt động ở PID 500 (ví dụ Windows Terminal), Hook xử lý bình thường
        let _ = hook.process_hook_key(&routing, 500, KeyEvent::ch('a'), 0x41);
        let (consume, batches) = hook.process_hook_key(&routing, 500, KeyEvent::ch('s'), 0x53);
        assert!(consume);
        assert_eq!(
            batches,
            vec![WinInputBatch::SendBackspaces(1), WinInputBatch::SendUnicodeText("á".into())]
        );

        // Khi TSF công bố đang hoạt động ở PID 600 (Notepad), Hook tự động nhường
        routing.publish_tsf_active(600, 0x1234);
        let (consume_tsf, batches_tsf) = hook.process_hook_key(&routing, 600, KeyEvent::ch('a'), 0x41);
        assert!(!consume_tsf);
        assert!(batches_tsf.is_empty());
    }
}
