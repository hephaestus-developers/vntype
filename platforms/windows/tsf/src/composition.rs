//! Quản lý vùng soạn thảo gạch chân (`ITfComposition`) và xử lý lỗi nhảy con trỏ của Notepad RichEdit.

/// Trạng thái của vùng soạn thảo TSF kèm chiến lược neo con trỏ sau khi kết thúc composition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompositionTracker {
    /// Cờ cho biết hiện có một `ITfComposition` đang mở trong `ITfContext` hay không.
    pub active: bool,
    /// Nội dung UTF-16 hiện tại nằm trong vùng composition.
    pub text_utf16: Vec<u16>,
    /// Vị trí con trỏ tính theo đơn vị mã UTF-16 từ đầu composition.
    pub cursor_utf16: u32,
    /// Cờ khoá chống gọi đệ quy khi chính Text Service chủ động gọi `EndComposition`.
    pub ending_internally: bool,
}

impl CompositionTracker {
    /// Cập nhật vùng composition với chuỗi mới và vị trí con trỏ mới.
    pub fn set_preedit(&mut self, text: &str, cursor_utf16: u32) {
        if text.is_empty() {
            self.clear();
            return;
        }
        self.active = true;
        self.text_utf16 = text.encode_utf16().collect();
        self.cursor_utf16 = cursor_utf16.min(self.text_utf16.len() as u32);
    }

    /// Đánh dấu kết thúc vùng composition và xoá bộ đệm.
    ///
    /// Trả về độ dài UTF-16 cuối cùng để `ITfRange` sao chép (`after->Collapse(TF_ANCHOR_END)`)
    /// đặt con trỏ chính xác về cuối từ, khắc phục hoàn toàn lỗi RichEdit trên Notepad Windows 11
    /// đưa con trỏ về đầu từ sau `EndComposition`.
    pub fn commit_and_end(&mut self, final_text: &str) -> u32 {
        let len_u16 = final_text.encode_utf16().count() as u32;
        self.clear();
        len_u16
    }

    /// Xoá trạng thái composition.
    pub fn clear(&mut self) {
        self.active = false;
        self.text_utf16.clear();
        self.cursor_utf16 = 0;
        self.ending_internally = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_tracker_utf16_and_clear() {
        let mut tr = CompositionTracker::default();
        tr.set_preedit("😀a", 10);
        assert!(tr.active);
        assert_eq!(tr.text_utf16.len(), 3);
        assert_eq!(tr.cursor_utf16, 3);
        assert_eq!(tr.commit_and_end("😀a"), 3);
        assert!(!tr.active);
    }
}
