//! Thông báo nổi trên màn hình (On-Screen Display - OSD) khi người dùng chuyển chế độ Việt/Anh.

/// Cấu hình cửa sổ nổi OSD hiển thị ngắn hạn giữa màn hình.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsdBanner {
    /// Ký hiệu ngắn (`"VN"` hoặc `"EN"`).
    pub badge: &'static str,
    /// Tiêu đề trạng thái (`"Tiếng Việt"` hoặc `"Tiếng Anh"`).
    pub title: &'static str,
    /// Thời gian tự động ẩn tính bằng mili-giây.
    pub duration_ms: u32,
}

impl OsdBanner {
    /// Tạo thông báo OSD cho trạng thái bật/tắt tiếng Việt mới.
    pub fn for_vietnamese_state(enabled: bool) -> Self {
        if enabled {
            OsdBanner { badge: "VN", title: "Tiếng Việt", duration_ms: 1200 }
        } else {
            OsdBanner { badge: "EN", title: "Tiếng Anh", duration_ms: 1200 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osd_banner_states() {
        assert_eq!(OsdBanner::for_vietnamese_state(true).badge, "VN");
        assert_eq!(OsdBanner::for_vietnamese_state(false).badge, "EN");
    }
}
