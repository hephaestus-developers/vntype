//! Điểm vào của trình nền `vntype-wayland` trên Linux.

pub mod protocol;
pub mod wayland_client;
pub mod xkb;

use vntype_config::{default_config_path, load_config_or_default};
use vntype_engine::Key;

use crate::protocol::WaylandProtocolDriver;
use crate::xkb::WaylandKeyInput;

fn print_help() {
    println!(
        "\
vntype-wayland {} — Trình nền Wayland IME cho VNType

Cách dùng:
  vntype-wayland [TUỲ CHỌN]

Tuỳ chọn:
  --probe        Kiểm tra biến môi trường WAYLAND_DISPLAY
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
        println!("vntype-wayland {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--probe") {
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        println!("WAYLAND_DISPLAY={wayland_display}");
        return;
    }

    let config_path = default_config_path();
    if args.iter().any(|a| a == "--self-test") {
        let cfg = load_config_or_default(config_path);
        let mut driver = WaylandProtocolDriver::new(cfg);
        driver.on_activate();
        for (i, ch) in "tieengs ".chars().enumerate() {
            let input = WaylandKeyInput {
                key: Key::Char(ch),
                keycode: 10 + i as u32,
                time: i as u32,
                pressed: true,
                ctrl: false,
                alt: false,
                meta: false,
            };
            let _ = driver.on_key(&input);
        }
        println!("vntype-wayland self-test OK");
        return;
    }

    if let Err(err) = wayland_client::run_wayland_daemon(config_path) {
        eprintln!("Lỗi khởi chạy vntype-wayland: {err}");
        std::process::exit(1);
    }
}
