//! Đọc tệp cấu hình và tính dấu vân tay dữ liệu bằng thuật toán băm BLAKE3.

use std::path::Path;

use crate::atomic_save::ConfigError;
use crate::schema::Config;

/// Tính mã băm BLAKE3 32-byte của một mảng byte để phát hiện thay đổi nội dung thực sự.
///
/// # Examples
///
/// ```
/// use vntype_config::fingerprint_bytes;
///
/// assert_eq!(fingerprint_bytes(b"abc"), fingerprint_bytes(b"abc"));
/// assert_ne!(fingerprint_bytes(b"abc"), fingerprint_bytes(b"abd"));
/// ```
pub fn fingerprint_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// Đọc và giải tuần tự hoá [`Config`] từ đường dẫn tệp `path`.
pub fn load_config_file(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path.as_ref())?;
    let cfg = serde_json::from_str::<Config>(&content)?;
    Ok(cfg)
}

/// Đọc cấu hình từ tệp `path`; nếu tệp chưa tồn tại hoặc lỗi định dạng thì trả về [`Config::default`].
pub fn load_config_or_default(path: impl AsRef<Path>) -> Config {
    load_config_file(path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_default() {
        let cfg = load_config_or_default("/nonexistent/vntype/missing_config.json");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn fingerprint_distinguishes_inputs() {
        assert_ne!(fingerprint_bytes(b"{}"), fingerprint_bytes(b"{\"a\":1}"));
    }
}
