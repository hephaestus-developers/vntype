//! Tính toán khoảng cách chỉnh sửa tối thiểu (xoá lùi + chèn mới) giữa hai chuỗi văn bản.

use alloc::string::String;
use alloc::vec::Vec;

/// Thao tác chỉnh sửa tối thiểu để biến chuỗi cũ trên màn hình thành chuỗi mới.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Số lượng ký tự (Unicode scalar `char`) cần xoá lùi từ vị trí con trỏ.
    pub delete_before: usize,
    /// Chuỗi ký tự mới cần chèn vào sau khi đã xoá lùi.
    pub insert: String,
}

/// So sánh hai chuỗi `old` và `new`, giữ lại tiền tố chung dài nhất và trả về lệnh [`Edit`].
///
/// # Examples
///
/// ```
/// use vntype_core::{diff, Edit};
///
/// assert_eq!(diff("tiêng", "tiếng"), Edit { delete_before: 3, insert: "ếng".into() });
/// ```
pub fn diff(old: &str, new: &str) -> Edit {
    let o: Vec<char> = old.chars().collect();
    let n: Vec<char> = new.chars().collect();
    let mut p = 0;
    while p < o.len().min(n.len()) && o[p] == n[p] {
        p += 1;
    }
    Edit { delete_before: o.len() - p, insert: n[p..].iter().collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_common_prefix() {
        assert_eq!(diff("tiêng", "tiếng"), Edit { delete_before: 3, insert: "ếng".into() });
        assert_eq!(diff("", "a"), Edit { delete_before: 0, insert: "a".into() });
        assert_eq!(diff("abc", ""), Edit { delete_before: 3, insert: String::new() });
    }
}
