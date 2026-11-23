//! Điểm vào của trình nền `vntype-hook` trên Windows.

#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

pub mod keyboard_hook;
pub mod tray;

use vntype_config::{default_config_path, load_config_or_default};
use vntype_engine::KeyEvent;
use vntype_win_ipc::SharedRoutingBlock;

use crate::keyboard_hook::WindowsHookProcessor;

fn print_help() {
    println!(
        "\
vntype-hook {} — Trình nền Windows Hook dự phòng cho VNType

Cách dùng:
  vntype-hook [TUỲ CHỌN]

Tuỳ chọn:
  --self-test    Chạy kiểm tra nội bộ nhanh
  -h, --help     Hiển thị trợ giúp
  -V, --version  Hiển thị phiên bản",
        env!("CARGO_PKG_VERSION")
    );
}

fn attach_parent_console_if_needed(has_cli_args: bool) {
    #[cfg(windows)]
    if has_cli_args {
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
    let _ = has_cli_args;
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_cli = args
        .iter()
        .any(|a| matches!(a.as_str(), "--help" | "-h" | "--version" | "-V" | "--self-test" | "--check"));
    attach_parent_console_if_needed(is_cli);

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("vntype-hook {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let cfg = load_config_or_default(default_config_path());
    let routing = SharedRoutingBlock::new();
    let mut hook = WindowsHookProcessor::new(cfg.clone());

    if args.iter().any(|a| a == "--self-test" || a == "--check") {
        let _ = hook.process_hook_key(&routing, 1, KeyEvent::ch('a'), 0x41);
        let (consumed, batches) = hook.process_hook_key(&routing, 1, KeyEvent::ch('s'), 0x53);
        assert!(consumed && !batches.is_empty());
        println!("vntype-hook self-test OK");
        return;
    }

    #[cfg(windows)]
    {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicIsize, Ordering};
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

        let bridge = vntype_settings::gui::win_gui::HookGuiBridge::new();
        let tray_hwnd = Arc::new(AtomicIsize::new(0));

        let bridge_for_hook = Arc::clone(&bridge);
        let hwnd_for_hook = Arc::clone(&tray_hwnd);
        let hook_thread = std::thread::spawn(move || {
            if let Err(err) =
                crate::keyboard_hook::win_hook::run_hook_event_loop(cfg, bridge_for_hook, hwnd_for_hook)
            {
                eprintln!("vntype-hook: lỗi khởi chạy Win32 hook (mã lỗi {err})");
            }
            std::process::exit(0);
        });

        let hwnd_for_toggle = Arc::clone(&tray_hwnd);
        let on_toggle = Box::new(move || {
            let raw = hwnd_for_toggle.load(Ordering::Acquire);
            if raw != 0 {
                unsafe {
                    PostMessageW(raw as HWND, crate::tray::win_tray::WM_TOGGLE_VN, 0, 0);
                }
            }
        });

        let hwnd_for_cfg = Arc::clone(&tray_hwnd);
        let on_config_changed = Box::new(move || {
            let raw = hwnd_for_cfg.load(Ordering::Acquire);
            if raw != 0 {
                unsafe {
                    PostMessageW(raw as HWND, crate::tray::win_tray::WM_WAKE_SYNC, 0, 0);
                }
            }
        });

        let hwnd_for_quit = Arc::clone(&tray_hwnd);
        let on_quit = Box::new(move || {
            let raw = hwnd_for_quit.load(Ordering::Acquire);
            if raw != 0 {
                unsafe {
                    PostMessageW(raw as HWND, crate::tray::win_tray::WM_QUIT_APP, 0, 0);
                }
            }
        });

        vntype_settings::gui::win_gui::run_embedded_gui(
            default_config_path(),
            bridge,
            on_toggle,
            on_config_changed,
            on_quit,
        );
        let raw = tray_hwnd.load(Ordering::Acquire);
        if raw != 0 {
            unsafe {
                PostMessageW(raw as HWND, crate::tray::win_tray::WM_QUIT_APP, 0, 0);
            }
        }
        let _ = hook_thread.join();
        std::process::exit(0);
    }

    #[cfg(not(windows))]
    {
        let _ = (hook, routing, cfg);
        eprintln!(
            "vntype-hook: chỉ chạy vòng lặp WH_KEYBOARD_LL trên Windows (hãy dùng --self-test để kiểm tra logic)."
        );
    }
}
