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
    use std::process::Command;
    use std::sync::atomic::Ordering;
    use windows_sys::Win32::Foundation::{CloseHandle, HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, GetKeyboardState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, SendInput, ToUnicode, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_LWIN, VK_MENU,
        VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
        HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PostQuitMessage, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    use vntype_config::{default_config_path, load_config_or_default, save_config};
    use vntype_engine::{Config, InputMethod, Key, KeyEvent, Modifiers};
    use vntype_win_ipc::win_shm::SharedRoutingMapping;

    use crate::osd::{OsdBanner, win_osd::WinOsdWindow};
    use crate::tray::{TrayMenuAction, TrayStatus, win_tray::WinTrayIcon};

    use super::{WinInputBatch, WindowsHookProcessor};

    const VNTYPE_INJECTED_TAG: usize = 0x564E_5450;
    const VK_OEM_COMMA: u32 = 0xBC;

    struct HookRuntime {
        processor: WindowsHookProcessor,
        config: Config,
        routing: SharedRoutingMapping,
        tray: Option<WinTrayIcon>,
        osd: Option<WinOsdWindow>,
        last_config_gen: u64,
        last_app: String,
    }

    thread_local! {
        static HOOK_RUNTIME: RefCell<Option<HookRuntime>> = const { RefCell::new(None) };
        static HOOK_HANDLE: RefCell<HHOOK> = const { RefCell::new(core::ptr::null_mut()) };
    }

    fn method_label(method: InputMethod) -> String {
        match method {
            InputMethod::Telex => "Telex".into(),
            InputMethod::Vni => "VNI".into(),
            InputMethod::SimpleTelex => "Simple Telex".into(),
        }
    }

    /// Gửi các batch backspace / Unicode tới ứng dụng đích qua `SendInput`.
    pub fn send_input_batches(batches: &[WinInputBatch]) {
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
        if !inputs.is_empty() {
            unsafe {
                SendInput(inputs.len() as u32, inputs.as_ptr(), core::mem::size_of::<INPUT>() as i32);
            }
        }
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

    fn current_modifiers() -> Modifiers {
        unsafe {
            let mut mods = Modifiers::empty();
            if (GetKeyState(VK_SHIFT as i32) as u16) & 0x8000 != 0 {
                mods |= Modifiers::SHIFT;
            }
            if (GetKeyState(VK_CONTROL as i32) as u16) & 0x8000 != 0 {
                mods |= Modifiers::CTRL;
            }
            if (GetKeyState(VK_MENU as i32) as u16) & 0x8000 != 0 {
                mods |= Modifiers::ALT;
            }
            if (GetKeyState(VK_LWIN as i32) as u16) & 0x8000 != 0
                || (GetKeyState(VK_RWIN as i32) as u16) & 0x8000 != 0
            {
                mods |= Modifiers::SUPER;
            }
            if (GetKeyState(VK_CAPITAL as i32) as u16) & 0x0001 != 0 {
                mods |= Modifiers::CAPS;
            }
            mods
        }
    }

    fn translate_vk_to_key_event(vk: u32, scan_code: u32) -> KeyEvent {
        let mods = current_modifiers();
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
        KeyEvent { key, mods }
    }

    fn sync_ui_and_actions(rt: &mut HookRuntime) {
        let cfg_gen = rt.routing.config_generation.load(Ordering::Acquire);
        if cfg_gen != rt.last_config_gen {
            rt.config = load_config_or_default(default_config_path());
            rt.processor.reload_config(rt.config.clone());
            rt.last_config_gen = cfg_gen;
            if let Some(tray) = &rt.tray {
                tray.update(TrayStatus {
                    vietnamese_enabled: rt.processor.is_enabled(),
                    input_method_label: method_label(rt.config.input_method),
                });
            }
        }

        let action = rt.tray.as_ref().and_then(|t| t.take_pending_action());
        if let Some(act) = action {
            match act {
                TrayMenuAction::ToggleVietnamese => {
                    let next = !rt.processor.is_enabled();
                    rt.processor.set_enabled(next);
                    if let Some(osd) = &rt.osd {
                        osd.show(OsdBanner::for_vietnamese_state(next));
                    }
                }
                TrayMenuAction::SelectTelex => {
                    rt.config.input_method = InputMethod::Telex;
                    let _ = save_config(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                }
                TrayMenuAction::SelectVni => {
                    rt.config.input_method = InputMethod::Vni;
                    let _ = save_config(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                }
                TrayMenuAction::SelectSimpleTelex => {
                    rt.config.input_method = InputMethod::SimpleTelex;
                    let _ = save_config(default_config_path(), &rt.config);
                    rt.processor.reload_config(rt.config.clone());
                    rt.last_config_gen = rt.routing.notify_config_reloaded();
                }
                TrayMenuAction::OpenSettingsWindow => {
                    let _ = Command::new("vntype-settings.exe").spawn();
                }
                TrayMenuAction::ExitApplication => unsafe {
                    PostQuitMessage(0);
                },
            }
            if let Some(tray) = &rt.tray {
                tray.update(TrayStatus {
                    vietnamese_enabled: rt.processor.is_enabled(),
                    input_method_label: method_label(rt.config.input_method),
                });
            }
        }
    }

    unsafe extern "system" fn ll_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let next_hook = HOOK_HANDLE.with(|h| *h.borrow());
        if code != HC_ACTION as i32 || (wparam as u32 != WM_KEYDOWN && wparam as u32 != WM_SYSKEYDOWN) {
            return unsafe { CallNextHookEx(next_hook, code, wparam, lparam) };
        }

        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        if (kb.flags & LLKHF_INJECTED) != 0 || kb.dwExtraInfo == VNTYPE_INJECTED_TAG {
            return unsafe { CallNextHookEx(next_hook, code, wparam, lparam) };
        }

        let event = translate_vk_to_key_event(kb.vkCode, kb.scanCode);
        let consume = HOOK_RUNTIME.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let Some(rt) = borrow.as_mut() else {
                return false;
            };
            sync_ui_and_actions(rt);

            // Phím tắt Alt+, để bật/tắt Tiếng Việt nhanh.
            if kb.vkCode == VK_OEM_COMMA
                && event.mods.contains(Modifiers::ALT)
                && !event.mods.contains(Modifiers::CTRL)
            {
                let next = !rt.processor.is_enabled();
                rt.processor.set_enabled(next);
                if let Some(osd) = &rt.osd {
                    osd.show(OsdBanner::for_vietnamese_state(next));
                }
                if let Some(tray) = &rt.tray {
                    tray.update(TrayStatus {
                        vietnamese_enabled: next,
                        input_method_label: method_label(rt.config.input_method),
                    });
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
                send_input_batches(&batches);
            }
            consumed
        });

        if consume { 1 } else { unsafe { CallNextHookEx(next_hook, code, wparam, lparam) } }
    }

    /// Khởi chạy vòng lặp thông điệp Win32 cho `WH_KEYBOARD_LL`, Tray và OSD.
    pub fn run_hook_event_loop(initial_config: Config) -> Result<(), u32> {
        let routing = SharedRoutingMapping::open_or_create()?;
        let last_config_gen = routing.config_generation.load(Ordering::Acquire);
        let tray = WinTrayIcon::new(TrayStatus {
            vietnamese_enabled: true,
            input_method_label: method_label(initial_config.input_method),
        });
        let osd = WinOsdWindow::new();
        let processor = WindowsHookProcessor::new(initial_config.clone());

        HOOK_RUNTIME.with(|cell| {
            *cell.borrow_mut() = Some(HookRuntime {
                processor,
                config: initial_config,
                routing,
                tray,
                osd,
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
