//! Bộ bảo vệ văn bản xung quanh (`surrounding_guard`) trước khi thực hiện thao tác xoá lùi (`Replace`).

use crate::offset::{is_word_codepoint, next_codepoint, utf16_prefix, utf16_suffix};

/// Ảnh chụp nhanh văn bản xung quanh con trỏ do ứng dụng đích cung cấp.
#[derive(Debug, Clone, Copy)]
pub struct SurroundingSnapshot<'a> {
    /// Toàn bộ chuỗi văn bản xung quanh mà ứng dụng báo cáo.
    pub text: &'a str,
    /// Vị trí con trỏ tính theo đơn vị mã UTF-16.
    pub cursor_utf16: u32,
    /// Vị trí điểm neo vùng chọn (anchor) tính theo đơn vị mã UTF-16.
    pub anchor_utf16: u32,
}

/// Kết quả xác minh thao tác xoá lùi hợp lệ, chứa sẵn số lượng cần xoá theo cả 3 đơn vị đo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceCheck {
    /// Chuỗi hậu tố thực sự sẽ bị xoá (UTF-8).
    pub deleted: String,
    /// Số lượng đơn vị mã UTF-16 cần xoá (cho Windows TSF, macOS IMK, Android).
    pub delete_utf16: u32,
    /// Số lượng ký tự Unicode (`char`) cần xoá (cho Linux Fcitx5, IBus).
    pub delete_chars: u32,
    /// Số lượng byte UTF-8 cần xoá (cho Linux Wayland `zwp_input_method_v2`).
    pub delete_bytes: u32,
}

/// Xác minh xem thao tác xoá lùi `delete_utf16` đơn vị mã trước con trỏ có an toàn tuyệt đối hay không.
///
/// Trả về `None` (tức không an toàn, phải chuyển sang `FailSafe`) trong các trường hợp:
/// 1. Đang có vùng bôi đen (`cursor_utf16 != anchor_utf16`).
/// 2. Hậu tố văn bản trước con trỏ không khớp với chuỗi `shown_before` mà bộ gõ tin là đang hiển thị.
/// 3. Con trỏ đang nằm ở giữa một từ (ký tự ngay sau con trỏ vẫn là ký tự thuộc từ).
///
/// # Examples
///
/// ```
/// use vntype_adapter::{check_replace, SurroundingSnapshot};
///
/// let snap = SurroundingSnapshot { text: "tiếng", cursor_utf16: 5, anchor_utf16: 5 };
/// let res = check_replace("tiếng", 3, snap).unwrap();
/// assert_eq!(res.delete_chars, 3);
/// ```
pub fn check_replace(
    shown_before: &str,
    delete_utf16: u32,
    surrounding: SurroundingSnapshot<'_>,
) -> Option<ReplaceCheck> {
    if delete_utf16 == 0 || surrounding.cursor_utf16 != surrounding.anchor_utf16 {
        return None;
    }

    let before_cursor = utf16_prefix(surrounding.text, surrounding.cursor_utf16)?;
    let shown = if shown_before.is_empty() { before_cursor } else { shown_before };
    let deleted = utf16_suffix(shown, delete_utf16)?;
    if deleted.is_empty() || !before_cursor.ends_with(deleted) {
        return None;
    }

    if let Some(next) = next_codepoint(surrounding.text, surrounding.cursor_utf16)
        && is_word_codepoint(next)
    {
        return None;
    }

    Some(ReplaceCheck {
        deleted: deleted.to_owned(),
        delete_utf16,
        delete_chars: deleted.chars().count() as u32,
        delete_bytes: deleted.len() as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(text: &str, cursor: u32, anchor: u32) -> SurroundingSnapshot<'_> {
        SurroundingSnapshot { text, cursor_utf16: cursor, anchor_utf16: anchor }
    }

    #[test]
    fn verified_replace_counts_all_units() {
        let check = check_replace("tiếng", 3, snapshot("tiếng", 5, 5)).unwrap();
        assert_eq!(check.deleted, "ếng");
        assert_eq!(check.delete_utf16, 3);
        assert_eq!(check.delete_chars, 3);
        assert_eq!(check.delete_bytes, "ếng".len() as u32);
    }

    #[test]
    fn rejects_selection_and_mid_word() {
        assert!(check_replace("tiếng", 3, snapshot("tiếng", 5, 2)).is_none());
        assert!(check_replace("", 1, snapshot("tiếng", 2, 2)).is_none());
    }
}
