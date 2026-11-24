//! Bộ điều khiển trạng thái giao diện cấu hình (`SettingsViewModel`) dùng chung cho mọi nền tảng.

use std::path::{Path, PathBuf};

use vntype_config::{
    Config, ConfigError, ToneStyle, UnicodeForm, ensure_macros_file, load_config_or_default,
    load_macros_file, save_config_atomic,
};
use vntype_core::{CHARSETS, InputMethodSpec, normalize_charset};

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

    /// Khôi phục bản thảo về đúng trạng thái đã lưu gần nhất.
    pub fn reset_draft_to_saved(&mut self) {
        self.draft = self.saved.clone();
    }

    /// Đặt lại toàn bộ bản thảo về cấu hình mặc định.
    pub fn reset_to_defaults(&mut self) {
        self.draft = Config::default();
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

    /// Đặt bảng mã mới; trả về `false` nếu định danh bảng mã không được hỗ trợ.
    pub fn set_charset(&mut self, charset: &str) -> bool {
        let normalized = normalize_charset(charset);
        if !CHARSETS.contains(&normalized) {
            return false;
        }
        self.draft.charset = normalized.to_string();
        self.draft.unicode_form =
            if normalized == "unicode_nfd" { UnicodeForm::Nfd } else { UnicodeForm::Nfc };
        true
    }

    /// Đặt quy tắc dấu thanh mới hoặc cũ.
    pub fn set_tone_style(&mut self, style: ToneStyle) {
        self.draft.tone_style = style;
    }

    /// Trả về đường dẫn tệp `macros.txt` tương ứng với tệp cấu hình hiện tại (ưu tiên `VNTYPE_MACROS`).
    pub fn macros_path(&self) -> PathBuf {
        macros_path_for_config(&self.config_path)
    }

    /// Đảm bảo tệp `macros.txt` tồn tại trên đĩa và trả về đường dẫn tới tệp đó.
    pub fn ensure_macros_file(&self) -> std::io::Result<PathBuf> {
        let path = self.macros_path();
        ensure_macros_file(&path)?;
        Ok(path)
    }

    /// Đếm số lượng quy tắc gõ tắt hợp lệ đang có trong `macros.txt`.
    pub fn macro_entry_count(&self) -> usize {
        load_macros_file(self.macros_path()).len()
    }

    /// Lưu bản thảo cấu hình xuống đĩa một cách nguyên tử.
    pub fn apply_and_save(&mut self) -> Result<(), ConfigError> {
        save_config_atomic(&self.config_path, &self.draft)?;
        self.saved = self.draft.clone();
        Ok(())
    }
}

/// Suy ra đường dẫn `macros.txt` nằm cùng thư mục với `config_path` (hoặc theo `VNTYPE_MACROS`).
pub fn macros_path_for_config(config_path: &Path) -> PathBuf {
    std::env::var_os("VNTYPE_MACROS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config_path.with_file_name("macros.txt"))
}

/// Kiểm tra tính hợp lệ của chuỗi JSON cấu hình và lưu nguyên tử xuống `config_path`.
pub fn validate_and_save_json(config_path: &Path, json: &str) -> Result<(), String> {
    let base = load_config_or_default(config_path);
    let patched = base.apply_json_patch(json).ok_or_else(|| "JSON cấu hình không hợp lệ".to_string())?;
    if InputMethodSpec::by_name(&patched.input_method).is_none() {
        return Err(format!("Kiểu gõ không hợp lệ: {}", patched.input_method));
    }
    let normalized_charset = normalize_charset(&patched.charset);
    if !CHARSETS.contains(&normalized_charset) {
        return Err(format!("Bảng mã không hợp lệ: {}", patched.charset));
    }
    save_config_atomic(config_path, &patched).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewmodel_validates_charset_method_and_macros() {
        let dir = std::env::temp_dir().join(format!("vntype_settings_core_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("config.json");

        let mut vm = SettingsViewModel::load(cfg_path.clone());
        assert!(!vm.is_dirty());
        assert!(!vm.set_input_method("unknown"));
        assert!(vm.set_input_method("vni"));
        assert!(vm.set_charset("tcvn3"));
        assert!(vm.is_dirty());

        vm.apply_and_save().unwrap();
        assert!(!vm.is_dirty());

        let macros_path = vm.ensure_macros_file().unwrap();
        assert!(macros_path.exists());
        assert_eq!(vm.macro_entry_count(), 0);

        std::fs::write(&macros_path, "ko=không\ndc=được\n").unwrap();
        assert_eq!(vm.macro_entry_count(), 2);

        validate_and_save_json(&cfg_path, r#"{"input_method":"telex"}"#).unwrap();
        let reloaded = SettingsViewModel::load(cfg_path);
        assert_eq!(reloaded.saved.input_method, "telex");
        assert!(validate_and_save_json(&reloaded.config_path, r#"{"input_method":"bad"}"#).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
