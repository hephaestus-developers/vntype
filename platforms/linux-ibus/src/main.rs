//! Điểm vào của trình nền `vntype-ibus` trên Linux.
//!
//! Hỗ trợ kết nối D-Bus thật tới IBus Daemon (`--ibus`) hoặc chạy ở chế độ kiểm thử IPC qua stdin.

pub mod dbus_service;
pub mod ibus_dbus;
pub mod keymap;

use std::io::{self, BufRead};
use vntype_config::default_config_path;

use crate::dbus_service::IbusEngineService;

fn print_help() {
    println!(
        "\
vntype-ibus {} — Trình nền IBus cho VNType

Cách dùng:
  vntype-ibus [TUỲ CHỌN]

Tuỳ chọn:
  --ibus         Kết nối D-Bus thật tới IBus Daemon
  --xml          In tệp mô tả thành phần IBus XML
  --self-test    Chạy kiểm tra nội bộ nhanh
  -h, --help     Hiển thị trợ giúp
  -V, --version  Hiển thị phiên bản",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config_path = default_config_path();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("vntype-ibus {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--xml") {
        print!("{}", include_str!("../data/vntype.xml.in"));
        return;
    }
    if args.iter().any(|a| a == "--self-test") {
        let mut svc = IbusEngineService::new(config_path);
        for b in b"Vieejt Nam " {
            svc.process_key_event(*b as u32, 0, 0);
        }
        println!("vntype-ibus self-test OK ({} tín hiệu)", svc.drain_signals().len());
        return;
    }
    if args.iter().any(|a| a == "--ibus") {
        if let Err(err) = ibus_dbus::run_ibus_daemon(config_path) {
            eprintln!("Lỗi khởi chạy vntype-ibus D-Bus: {err}");
            std::process::exit(1);
        }
        return;
    }

    // Vòng lặp xử lý sự kiện dòng lệnh / IPC kiểm thử
    let mut service = IbusEngineService::new(config_path);
    let stdin = io::stdin();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed == "QUIT" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("KEY ")
            && let Ok(keyval) = rest.parse::<u32>()
        {
            let handled = service.process_key_event(keyval, 0, 0);
            println!("HANDLED={} SIGNALS={:?}", handled, service.drain_signals());
        }
    }
}
