//! Ứng dụng quản lý và kiểm tra cấu hình `vntype-settings` cho Windows và Linux.

use std::path::PathBuf;

use vntype_config::{
    Config, ConfigError, ToneStyle, default_config_path, load_config_or_default, save_config_atomic,
};
use vntype_core::InputMethodSpec;

/// Bộ điều khiển trạng thái giao diện cấu hình (ViewModel), hỗ trợ kiểm tra hợp lệ và lưu nguyên tử.
#[derive(Debug, Clone)]
pub struct SettingsViewModel {
    /// Đường dẫn tệp cấu hình đang chỉnh sửa.
    pub config_path: PathBuf,
    /// Bản thảo cấu hình đang chỉnh sửa trên giao diện.
    pub draft: Config,
    /// Bản sao cấu hình gốc đã lưu trên đĩa để phát hiện thay đổi.
    pub saved: Config,
}

impl SettingsViewModel {
    /// Tải cấu hình từ đường dẫn `config_path` để khởi tạo ViewModel.
    pub fn load(config_path: PathBuf) -> Self {
        let cfg = load_config_or_default(&config_path);
        SettingsViewModel { config_path, draft: cfg.clone(), saved: cfg }
    }

    /// Kiểm tra xem người dùng đã thay đổi bất kỳ thông số nào so với bản đã lưu hay chưa.
    pub fn is_dirty(&self) -> bool {
        self.draft != self.saved
    }

    /// Đặt kiểu gõ mới (`"telex"`, `"simple_telex"`, `"vni"`, `"viqr"`).
    ///
    /// Trả về `false` nếu tên kiểu gõ không hợp lệ.
    pub fn set_input_method(&mut self, method: &str) -> bool {
        if InputMethodSpec::by_name(method).is_none() {
            return false;
        }
        self.draft.input_method = method.to_string();
        true
    }

    /// Đặt quy tắc dấu thanh mới hoặc cũ.
    pub fn set_tone_style(&mut self, style: ToneStyle) {
        self.draft.tone_style = style;
    }

    /// Lưu bản thảo cấu hình xuống đĩa một cách nguyên tử.
    pub fn apply_and_save(&mut self) -> Result<(), ConfigError> {
        save_config_atomic(&self.config_path, &self.draft)?;
        self.saved = self.draft.clone();
        Ok(())
    }
}

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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
    println!("VNType Settings ({}) — Kiểu gõ: {}", vm.config_path.display(), vm.draft.input_method);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

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
