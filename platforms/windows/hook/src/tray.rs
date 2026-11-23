//! Quản lý biểu tượng khay hệ thống (System Tray Icon) và thực đơn ngữ cảnh nhanh trên Windows.

/// Các lệnh điều khiển người dùng có thể kích hoạt từ biểu tượng khay hệ thống.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    /// Bật hoặc tắt chế độ gõ tiếng Việt (`Alt+,`).
    ToggleVietnamese,
    /// Chọn kiểu gõ Telex.
    SelectTelex,
    /// Chọn kiểu gõ VNI.
    SelectVni,
    /// Chọn kiểu gõ Simple Telex.
    SelectSimpleTelex,
    /// Mở cửa sổ giao diện cấu hình `vntype-settings`.
    OpenSettingsWindow,
    /// Thoát trình nền VNType.
    ExitApplication,
}

/// Trạng thái hiển thị của biểu tượng khay hệ thống.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayStatus {
    /// `true` nếu đang ở chế độ Tiếng Việt (hiển thị nhãn `VN`), `false` nếu Tiếng Anh (`EN`).
    pub vietnamese_enabled: bool,
    /// Tên kiểu gõ hiện tại (`"Telex"`, `"VNI"`, ...).
    pub input_method_label: String,
}

impl TrayStatus {
    /// Tạo chuỗi chú thích (Tooltip) hiển thị khi di chuột lên biểu tượng khay hệ thống.
    pub fn tooltip_text(&self) -> String {
        let mode = if self.vietnamese_enabled { "Tiếng Việt (VN)" } else { "Tiếng Anh (EN)" };
        format!("VNType — {} | Kiểu gõ: {} (Alt+, để chuyển)", mode, self.input_method_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_tooltip_contains_mode_and_method() {
        let s = TrayStatus { vietnamese_enabled: true, input_method_label: "Telex".into() };
        assert!(s.tooltip_text().contains("VN") && s.tooltip_text().contains("Telex"));
    }
}
