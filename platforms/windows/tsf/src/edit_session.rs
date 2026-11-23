//! Thực thi danh sách hiệu ứng nguyên tử [`vntype_adapter::Effect`] bên trong một phiên chỉnh sửa
//! đồng bộ `ITfEditSession::DoEditSession(TfEditCookie)`.

use vntype_adapter::Effect;

use crate::composition::CompositionTracker;

/// Giao diện trừu tượng hoá tài liệu `ITfContext` của Windows TSF trong một phiên `TfEditCookie`.
///
/// Tách giao diện này giúp kiểm thử toàn bộ quy trình `SetPreedit`, `CommitText`, `ReplaceText`
/// và workaround Notepad RichEdit trên mọi hệ điều hành mà không cần khởi chạy tiến trình COM.
pub trait TsfContextBackend {
    /// Đặt văn bản vào vùng `ITfComposition` đang mở (hoặc tạo mới nếu chưa có) và tô gạch chân.
    fn set_composition_text(&mut self, text: &str, cursor_utf16: u32);
    /// Kết thúc `ITfComposition` hiện tại và neo lại con trỏ ở cuối vùng vừa chốt.
    fn end_composition_at_end(&mut self);
    /// Chèn trực tiếp chuỗi văn bản tại vị trí con trỏ và đóng composition nếu đang mở.
    fn commit_text_at_selection(&mut self, text: &str);
    /// Dịch điểm bắt đầu của vùng chọn lùi `delete_utf16` đơn vị mã (`ShiftStart`) và ghi đè bằng `insert`.
    fn replace_before_caret(&mut self, delete_utf16: u32, insert: &str);
}

/// Thực thi danh sách hiệu ứng nguyên tử `effects` lên `backend` trong một phiên chỉnh sửa TSF.
///
/// Trả về `true` (`pfEaten = TRUE`) nếu sự kiện phím đã được bộ gõ xử lý, hoặc `false` (`pfEaten = FALSE`)
/// nếu có hiệu ứng `Effect::ForwardKey` yêu cầu Windows chuyển phím gốc cho ứng dụng.
pub fn apply_effects_in_edit_session<B: TsfContextBackend>(
    effects: &[Effect],
    tracker: &mut CompositionTracker,
    backend: &mut B,
) -> bool {
    let mut forward_original = false;

    for eff in effects {
        match eff {
            Effect::SetPreedit { text, cursor_utf16, .. } => {
                tracker.set_preedit(text, *cursor_utf16);
                backend.set_composition_text(text, *cursor_utf16);
            }
            Effect::ClearPreedit => {
                if tracker.active {
                    tracker.clear();
                    backend.end_composition_at_end();
                }
            }
            Effect::CommitText { text } => {
                tracker.commit_and_end(text);
                backend.commit_text_at_selection(text);
            }
            Effect::ReplaceText { delete_utf16, insert, .. } => {
                if tracker.active {
                    tracker.clear();
                    backend.end_composition_at_end();
                }
                backend.replace_before_caret(*delete_utf16, insert);
            }
            Effect::WaitMs { .. } => {}
            Effect::ForwardKey { .. } => {
                forward_original = true;
            }
        }
    }

    !effects.is_empty() && !forward_original
}
