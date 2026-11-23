//! Định nghĩa các kiểu dữ liệu `#[repr(C)]` và các hằng số giao thức cho giao diện C ABI.

use std::ffi::c_char;
use std::ptr;

/// Chế độ xuất chữ: hiển thị vùng gạch chân tạm thời (Preedit).
pub const VNT_MODE_PREEDIT: u32 = 0;
/// Chế độ xuất chữ: thay thế trực tiếp bằng lệnh xoá lùi (Replace).
pub const VNT_MODE_REPLACE: u32 = 1;

/// Trường nhập liệu văn bản thông thường.
pub const VNT_FIELD_TEXT: u32 = 0;
/// Trường nhập liệu mật khẩu hoặc mã PIN nhạy cảm.
pub const VNT_FIELD_PASSWORD: u32 = 1;
/// Trường chỉ nhập số.
pub const VNT_FIELD_NUMBER: u32 = 2;
/// Trường nhập địa chỉ thư điện tử.
pub const VNT_FIELD_EMAIL: u32 = 3;
/// Thanh địa chỉ trình duyệt / Omnibox.
pub const VNT_FIELD_URL: u32 = 4;
/// Giao diện dòng lệnh Terminal.
pub const VNT_FIELD_TERMINAL: u32 = 5;

/// Hiệu ứng: đặt chuỗi gạch chân tạm thời (`SetPreedit`).
pub const VNT_EFFECT_SET_PREEDIT: u32 = 1;
/// Hiệu ứng: xoá vùng gạch chân tạm thời (`ClearPreedit`).
pub const VNT_EFFECT_CLEAR_PREEDIT: u32 = 2;
/// Hiệu ứng: chốt chuỗi văn bản chính thức vào ứng dụng (`CommitText`).
pub const VNT_EFFECT_COMMIT_TEXT: u32 = 3;
/// Hiệu ứng: xoá lùi văn bản trước con trỏ và chèn chuỗi mới (`ReplaceText`).
pub const VNT_EFFECT_REPLACE_TEXT: u32 = 4;
/// Hiệu ứng: yêu cầu bộ điều phối sự kiện chờ `wait_ms` mili-giây (`WaitMs`).
pub const VNT_EFFECT_WAIT_MS: u32 = 5;
/// Hiệu ứng: chuyển tiếp phím gốc về cho ứng dụng (`ForwardKey`).
pub const VNT_EFFECT_FORWARD_KEY: u32 = 6;

/// Cấu trúc C biểu diễn một hiệu ứng nguyên tử.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VntEffect {
    /// Mã phân loại hiệu ứng (`VNT_EFFECT_*`).
    pub kind: u32,
    /// Số lượng đơn vị mã UTF-16 cần xoá lùi.
    pub delete_utf16: u32,
    /// Số lượng ký tự Unicode (`char`) cần xoá lùi.
    pub delete_chars: u32,
    /// Số lượng byte UTF-8 cần xoá lùi.
    pub delete_bytes: u32,
    /// Vị trí con trỏ theo đơn vị mã UTF-16.
    pub cursor_utf16: u32,
    /// Vị trí con trỏ theo số byte UTF-8.
    pub cursor_bytes: u32,
    /// Số mili-giây cần chờ trong giao dịch bất đồng bộ.
    pub wait_ms: u32,
    /// Mã phím vật lý hoặc định danh sự kiện phím gốc.
    pub raw_keycode: u32,
    /// Con trỏ chuỗi UTF-8 kết thúc NUL (hoặc `NULL` nếu hiệu ứng không mang chuỗi).
    pub text: *const c_char,
}

impl Default for VntEffect {
    fn default() -> Self {
        VntEffect {
            kind: 0,
            delete_utf16: 0,
            delete_chars: 0,
            delete_bytes: 0,
            cursor_utf16: 0,
            cursor_bytes: 0,
            wait_ms: 0,
            raw_keycode: 0,
            text: ptr::null(),
        }
    }
}

/// Cấu trúc C biểu diễn mảng các hiệu ứng nguyên tử trả về từ một thao tác.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VntEffectList {
    /// Con trỏ tới phần tử đầu tiên của mảng hiệu ứng.
    pub effects: *const VntEffect,
    /// Số lượng phần tử trong mảng.
    pub count: u32,
}

impl VntEffectList {
    /// Danh sách hiệu ứng rỗng.
    pub const EMPTY: VntEffectList = VntEffectList { effects: ptr::null(), count: 0 };
}
