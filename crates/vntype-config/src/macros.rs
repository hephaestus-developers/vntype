//! Đọc và khởi tạo tệp bảng gõ tắt văn bản thuần (`macros.txt`).

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Số lượng quy tắc gõ tắt tối đa được nạp từ `macros.txt`.
pub const MAX_MACRO_ENTRIES: usize = 10_000;

/// Kích thước tệp `macros.txt` tối đa được phép đọc (2 MiB) để tránh quá tải bộ nhớ.
pub const MAX_MACROS_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Nội dung mẫu khi khởi tạo tệp `macros.txt` lần đầu.
pub const DEFAULT_MACROS_TEMPLATE: &str = "\
# Bảng gõ tắt (Macro) của VNType
# Mỗi dòng một quy tắc theo định dạng: tu_viet_tat=cụm từ mở rộng
# Các dòng trống hoặc bắt đầu bằng # sẽ được bỏ qua.
#
# Ví dụ:
# ko=không
# dc=được
# hcm=Thành phố Hồ Chí Minh
";

/// Phân tích nội dung tệp `macros.txt` thành bảng ánh xạ `từ_viết_tắt -> chuỗi_mở_rộng`.
pub fn parse_macros_text(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim();
        if key.is_empty() || val.is_empty() {
            continue;
        }
        map.insert(key.to_string(), val.to_string());
        if map.len() >= MAX_MACRO_ENTRIES {
            break;
        }
    }
    map
}

/// Đọc bảng gõ tắt từ tệp `macros.txt`; trả về bảng rỗng nếu tệp chưa tồn tại, quá kích thước hoặc không đọc được.
pub fn load_macros_file(path: impl AsRef<Path>) -> HashMap<String, String> {
    let path = path.as_ref();
    let Ok(meta) = std::fs::metadata(path) else {
        return HashMap::new();
    };
    if meta.len() > MAX_MACROS_FILE_BYTES {
        return HashMap::new();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => parse_macros_text(&content),
        Err(_) => HashMap::new(),
    }
}

/// Đảm bảo tệp `macros.txt` tồn tại trên đĩa; nếu chưa có thì tạo mới với [`DEFAULT_MACROS_TEMPLATE`].
pub fn ensure_macros_file(path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file.write_all(DEFAULT_MACROS_TEMPLATE.as_bytes()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_macros_handles_comments_blanks_and_equals_in_value() {
        let input = "\
# Ghi chú
  # Ghi chú thụt lề

ko = không
dc=được
url=https://example.com/?a=1&b=2
invalid_line_without_eq
=missing_key
empty_val=
";
        let map = parse_macros_text(input);
        assert_eq!(map.get("ko").map(String::as_str), Some("không"));
        assert_eq!(map.get("dc").map(String::as_str), Some("được"));
        assert_eq!(map.get("url").map(String::as_str), Some("https://example.com/?a=1&b=2"));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn ensure_macros_creates_template_without_overwriting() {
        let dir = std::env::temp_dir().join(format!("vntype_macros_test_{}", std::process::id()));
        let file = dir.join("macros.txt");
        let _ = std::fs::remove_dir_all(&dir);

        ensure_macros_file(&file).unwrap();
        let initial = std::fs::read_to_string(&file).unwrap();
        assert!(initial.contains("VNType"));

        std::fs::write(&file, "ko=không\n").unwrap();
        ensure_macros_file(&file).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "ko=không\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_macros_caps_at_max_entries() {
        let mut input = String::new();
        for i in 0..(MAX_MACRO_ENTRIES + 50) {
            input.push_str(&format!("k{i}=v{i}\n"));
        }
        let map = parse_macros_text(&input);
        assert_eq!(map.len(), MAX_MACRO_ENTRIES);
    }
}
