//! Ghi tệp cấu hình nguyên tử (Atomic File Write) để bảo vệ dữ liệu người dùng.
//!
//! Quá trình ghi luôn tạo một tệp tạm duy nhất trong cùng thư mục, đẩy dữ liệu xuống đĩa (`sync_all`),
//! rồi thực hiện lệnh đổi tên nguyên tử (`std::fs::rename`) đè lên tệp đích.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::schema::Config;

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Các loại lỗi có thể xảy ra khi đọc hoặc ghi tệp cấu hình.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Lỗi hệ thống tệp tin từ hệ điều hành.
    #[error("Lỗi đọc/ghi tệp cấu hình: {0}")]
    Io(#[from] std::io::Error),
    /// Lỗi cú pháp hoặc kiểu dữ liệu JSON.
    #[error("Lỗi định dạng JSON cấu hình: {0}")]
    Json(#[from] serde_json::Error),
}

fn unique_tmp_path(target: &Path) -> PathBuf {
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    target.with_file_name(format!(".{name}.{pid}.{seq}.tmp"))
}

/// Lưu cấu hình `config` xuống đường dẫn `path` một cách nguyên tử.
///
/// # Examples
///
/// ```no_run
/// use vntype_config::{save_config_atomic, Config};
///
/// save_config_atomic("/tmp/vntype-example-config.json", &Config::default()).unwrap();
/// ```
pub fn save_config_atomic(path: impl AsRef<Path>, config: &Config) -> Result<(), ConfigError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(config)?;
    let tmp_path = unique_tmp_path(path);

    let write_res = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(serialized.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err.into());
    }

    if let Err(err) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load_config_file;

    #[test]
    fn atomic_save_and_load_roundtrip() {
        let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("vntype_config_test_{}_{}", std::process::id(), seq));
        let path = dir.join("config.json");
        let cfg = Config { input_method: "vni".into(), ..Config::default() };
        save_config_atomic(&path, &cfg).unwrap();
        let loaded = load_config_file(&path).unwrap();
        assert_eq!(loaded.input_method, "vni");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
