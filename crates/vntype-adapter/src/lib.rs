//! Bộ điều phối Adapter đa nền tảng (`vntype-adapter`).
//!
//! Crate này gom toàn bộ logic dễ gây lỗi nhất của các bộ gõ trên mọi hệ điều hành về một nơi duy nhất:
//! - [`offset`]: Quy đổi chính xác giữa Byte UTF-8, Unicode scalar (`char`) và đơn vị mã UTF-16.
//! - [`field_policy`]: Chính sách bảo mật và an toàn cho ô mật khẩu, số, email, thanh địa chỉ URL và terminal.
//! - [`surrounding_guard`]: Kiểm tra an toàn văn bản xung quanh trước khi xoá lùi (`Replace`).
//! - [`planner`]: Lập kế hoạch thi hành (`Plan`) cùng thời gian trễ (`wait_ms`) từ `AppProfile`.
//! - [`controller`]: Máy trạng thái [`AdapterController`] phát sinh danh sách hiệu ứng nguyên tử ([`Effect`]),
//!   tự động xử lý chốt chữ trước khi huỷ trong trường hợp `FailSafe` và quản lý hàng đợi giao dịch.

pub mod controller;
pub mod field_policy;
pub mod offset;
pub mod planner;
pub mod surrounding_guard;

pub use controller::{AdapterController, Effect, QueuedKey};
pub use field_policy::{is_disabled, is_sensitive, is_terminal};
pub use offset::{
    char_suffix, char_suffix_utf16_len, char_to_byte_offset, char_to_utf16_offset, is_word_codepoint,
    next_codepoint, utf16_len, utf16_prefix, utf16_suffix, utf16_to_byte_offset, utf16_to_char_offset,
};
pub use planner::{
    AdapterAction, DeleteAmount, MAX_REPLACE_WAIT_MS, MIN_REPLACE_WAIT_MS, POST_COMMIT_DELAY_MS, Plan,
    PlanContext, PlanKind, ReplaceFallback, plan_action, plan_action_with_fallback, replace_wait_ms,
    utf16_cursor_to_byte,
};
pub use surrounding_guard::{ReplaceCheck, SurroundingSnapshot, check_replace};
