//! Xác định đường dẫn tệp cấu hình và từ điển cá nhân theo quy chuẩn của từng hệ điều hành.

use std::path::PathBuf;

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty()).map(PathBuf::from)
}

/// Trả về thư mục lưu trữ cấu hình mặc định của VNType trên hệ điều hành hiện tại.
pub fn default_config_dir() -> PathBuf {
    if let Some(custom) = env_path("VNTYPE_CONFIG_DIR") {
        return custom;
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = env_path("HOME") {
        return home.join("Library").join("Application Support").join("VNType");
    }
    #[cfg(target_os = "windows")]
    if let Some(appdata) = env_path("APPDATA") {
        return appdata.join("VNType");
    }
    if let Some(xdg) = env_path("XDG_CONFIG_HOME") {
        return xdg.join("vntype");
    }
    if let Some(home) = env_path("HOME") {
        return home.join(".config").join("vntype");
    }
    PathBuf::from(".")
}

/// Trả về đường dẫn tệp `config.json` mặc định (có hỗ trợ biến môi trường `VNTYPE_CONFIG`).
pub fn default_config_path() -> PathBuf {
    env_path("VNTYPE_CONFIG").unwrap_or_else(|| default_config_dir().join("config.json"))
}

/// Trả về đường dẫn tệp từ điển cá nhân `user_words.txt` mặc định (có hỗ trợ `VNTYPE_USER_WORDS`).
pub fn default_user_words_path() -> PathBuf {
    env_path("VNTYPE_USER_WORDS").unwrap_or_else(|| default_config_dir().join("user_words.txt"))
}

/// Trả về đường dẫn tệp bảng gõ tắt `macros.txt` mặc định (có hỗ trợ biến môi trường `VNTYPE_MACROS`).
pub fn default_macros_path() -> PathBuf {
    env_path("VNTYPE_MACROS").unwrap_or_else(|| default_config_dir().join("macros.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_have_expected_filenames() {
        assert_eq!(default_config_path().file_name().and_then(|s| s.to_str()), Some("config.json"));
        assert_eq!(default_user_words_path().file_name().and_then(|s| s.to_str()), Some("user_words.txt"));
        assert_eq!(default_macros_path().file_name().and_then(|s| s.to_str()), Some("macros.txt"));
    }
}
