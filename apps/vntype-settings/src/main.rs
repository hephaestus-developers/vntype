//! Điểm vào dòng lệnh và GUI cho `vntype-settings`.

#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

use vntype_config::default_config_path;
use vntype_settings::SettingsViewModel;

fn print_help() {
    println!(
        "\
vntype-settings {} — Quản lý cấu hình VNType

Cách dùng:
  vntype-settings [TUỲ CHỌN]

Tuỳ chọn:
  --print                In cấu hình hiện tại dưới dạng JSON
  --set-method <KIỂU_GÕ> Cập nhật và lưu kiểu gõ (telex | simple_telex | vni | viqr)
  -h, --help             Hiển thị trợ giúp
  -V, --version          Hiển thị phiên bản",
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
        .any(|a| matches!(a.as_str(), "--help" | "-h" | "--version" | "-V" | "--print" | "--set-method"));
    attach_parent_console_if_needed(is_cli);

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("vntype-settings {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let path = default_config_path();
    let mut vm = SettingsViewModel::load(path);

    if args.iter().any(|a| a == "--print") {
        println!("{}", serde_json::to_string_pretty(&vm.draft).unwrap_or_default());
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--set-method") {
        let Some(m) = args.get(pos + 1) else {
            eprintln!("Thiếu giá trị cho --set-method");
            std::process::exit(2);
        };
        if !vm.set_input_method(m) {
            eprintln!("Kiểu gõ không hợp lệ: {m} (hỗ trợ: telex, simple_telex, vni, viqr)");
            std::process::exit(2);
        }
        if let Err(err) = vm.apply_and_save() {
            eprintln!("Không thể lưu tệp cấu hình: {err}");
            std::process::exit(1);
        }
        println!("Đã lưu kiểu gõ: {m}");
        return;
    }

    #[cfg(windows)]
    {
        vntype_settings::gui::win_gui::run_settings_gui(vm);
    }

    #[cfg(not(windows))]
    {
        println!("VNType Settings ({}) — Kiểu gõ: {}", vm.config_path.display(), vm.draft.input_method);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use vntype_config::ToneStyle;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn viewmodel_validates_tracks_dirty_and_saves() {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("vntype_settings_test_{}_{}", std::process::id(), seq));
        let path = dir.join("config.json");

        let mut vm = SettingsViewModel::load(path.clone());
        assert!(!vm.is_dirty());
        assert!(!vm.set_input_method("invalid_method"));
        assert!(!vm.is_dirty());

        assert!(vm.set_input_method("vni"));
        vm.set_tone_style(ToneStyle::Old);
        assert!(vm.is_dirty());

        vm.apply_and_save().unwrap();
        assert!(!vm.is_dirty());

        let reloaded = SettingsViewModel::load(path);
        assert_eq!(reloaded.saved.input_method, "vni");
        assert_eq!(reloaded.saved.tone_style, ToneStyle::Old);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
