//! Khai báo tập tính năng hiển thị trên giao diện Settings theo từng hệ điều hành.

use serde::{Deserialize, Serialize};

/// Tập hợp cờ điều khiển các thành phần UI đặc thù theo hệ điều hành.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    /// Cho phép bật/tắt tuỳ chọn khởi động cùng hệ thống (Windows/macOS).
    pub supports_autostart: bool,
    /// Cho phép kết nối IPC kiểm tra trạng thái tiến trình nền / TSF / Hook (Windows).
    pub supports_daemon_ipc: bool,
    /// Hiển thị trạng thái khung bộ gõ hệ thống như IBus / Fcitx5 (Linux).
    pub supports_input_framework_status: bool,
    /// Hiển thị kiểm tra quyền Accessibility / Input Monitoring (macOS).
    pub supports_accessibility_check: bool,
    /// Cho phép mở và theo dõi tệp `macros.txt` bằng trình soạn thảo mặc định của hệ thống.
    pub supports_macro_file_editor: bool,
}

impl PlatformCapabilities {
    /// Cấu hình khả năng giao diện cho Windows.
    pub const fn windows() -> Self {
        Self {
            supports_autostart: true,
            supports_daemon_ipc: true,
            supports_input_framework_status: false,
            supports_accessibility_check: false,
            supports_macro_file_editor: true,
        }
    }

    /// Cấu hình khả năng giao diện cho Linux (IBus / Fcitx5 quản lý việc kích hoạt bộ gõ).
    pub const fn linux() -> Self {
        Self {
            supports_autostart: false,
            supports_daemon_ipc: false,
            supports_input_framework_status: true,
            supports_accessibility_check: false,
            supports_macro_file_editor: true,
        }
    }

    /// Cấu hình khả năng giao diện cho macOS.
    pub const fn macos() -> Self {
        Self {
            supports_autostart: false,
            supports_daemon_ipc: false,
            supports_input_framework_status: false,
            supports_accessibility_check: true,
            supports_macro_file_editor: true,
        }
    }

    /// Trả về tập khả năng tương ứng với hệ điều hành đang biên dịch.
    pub const fn for_current_os() -> Self {
        if cfg!(target_os = "windows") {
            Self::windows()
        } else if cfg!(target_os = "macos") {
            Self::macos()
        } else {
            Self::linux()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_differ_by_platform() {
        let win = PlatformCapabilities::windows();
        let linux = PlatformCapabilities::linux();
        let mac = PlatformCapabilities::macos();

        assert!(win.supports_autostart);
        assert!(win.supports_daemon_ipc);
        assert!(!linux.supports_autostart);
        assert!(linux.supports_input_framework_status);
        assert!(!mac.supports_autostart);
        assert!(mac.supports_accessibility_check);
        assert!(
            win.supports_macro_file_editor
                && linux.supports_macro_file_editor
                && mac.supports_macro_file_editor
        );
    }
}
