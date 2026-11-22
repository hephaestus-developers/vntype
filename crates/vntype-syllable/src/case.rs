//! Các hàm chuyển đổi chữ hoa và chữ thường cho ký tự đơn tiếng Việt trong môi trường `no_std`.

/// Chuyển một ký tự sang dạng chữ thường.
///
/// Nếu kết quả chuyển đổi của Unicode sinh ra nhiều hơn một ký tự, hàm giữ nguyên ký tự gốc.
#[inline]
pub fn lower(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Chuyển một ký tự sang dạng chữ hoa.
///
/// Nếu kết quả chuyển đổi của Unicode sinh ra nhiều hơn một ký tự, hàm giữ nguyên ký tự gốc.
#[inline]
pub fn upper(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_conversion_vietnamese() {
        assert_eq!(lower('Đ'), 'đ');
        assert_eq!(upper('đ'), 'Đ');
        assert_eq!(lower('Ế'), 'ế');
        assert_eq!(upper('ự'), 'Ự');
    }
}
