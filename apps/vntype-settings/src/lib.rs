//! Thư viện quản lý và giao diện cấu hình `vntype-settings` cho Windows và Linux.

pub mod gui;

use std::path::PathBuf;

use vntype_config::{
    Config, ConfigError, ToneStyle, load_config_or_default, save_config_atomic,
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
