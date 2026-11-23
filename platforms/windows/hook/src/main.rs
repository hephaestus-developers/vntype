//! Điểm vào của trình nền `vntype-hook` trên Windows.

pub mod keyboard_hook;
pub mod osd;
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        if let Err(err) = crate::keyboard_hook::win_hook::run_hook_event_loop(cfg) {
            eprintln!("vntype-hook: lỗi khởi chạy Win32 hook (mã lỗi {err})");
            std::process::exit(1);
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (hook, routing, cfg);
        eprintln!(
            "vntype-hook: chỉ chạy vòng lặp WH_KEYBOARD_LL trên Windows (hãy dùng --self-test để kiểm tra logic)."
        );
    }
}
