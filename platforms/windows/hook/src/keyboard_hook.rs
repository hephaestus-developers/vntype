//! Bộ điều khiển Hook bàn phím mức thấp (`WH_KEYBOARD_LL`) + `SendInput` dự phòng trên Windows.
//!
//! Bộ điều khiển này sử dụng [`vntype_adapter::AdapterController`] ở chế độ `OutputMode::Replace`
//! với `allow_unverified_replace = true`, và tự động nhường quyền (`bypass`) khi vùng nhớ dùng chung
//! [`vntype_win_ipc::SharedRoutingBlock`] báo rằng TSF đang kiểm soát tiến trình hiện hành.

use vntype_adapter::{AdapterController, Effect};
use vntype_engine::{Config, KeyEvent, OutputMode};
use vntype_win_ipc::SharedRoutingBlock;

/// Lệnh tổng hợp bàn phím (`SendInput`) phát ra từ trình nền Hook trên Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinInputBatch {
    /// Gửi `count` lần phím Backspace (`VK_BACK`) để xoá các ký tự cũ.
    SendBackspaces(u32),
    /// Gửi chuỗi ký tự Unicode qua cờ `KEYEVENTF_UNICODE`.
    SendUnicodeText(String),
}

/// Bộ xử lý sự kiện phím cho trình nền Windows Hook.
pub struct WindowsHookProcessor {
    controller: AdapterController,
}

impl WindowsHookProcessor {
    /// Khởi tạo bộ xử lý Windows Hook với cấu hình `config`.
    pub fn new(config: Config) -> Self {
        let mut controller = AdapterController::new(config, OutputMode::Replace);
        controller.set_allow_unverified_replace(true);
        WindowsHookProcessor { controller }
    }

    /// Xử lý một sự kiện phím từ `WH_KEYBOARD_LL`.
    ///
    /// Nếu `routing` cho biết tiến trình `foreground_pid` đang được `vntype-tsf` xử lý ở chế độ Preedit,
    /// hàm lập tức trả về `(false, Vec::new())` để nhường hoàn toàn cho TSF.
    pub fn process_hook_key(
        &mut self,
        routing: &SharedRoutingBlock,
        foreground_pid: u32,
        event: KeyEvent,
        vk_code: u32,
    ) -> (bool, Vec<WinInputBatch>) {
        if routing.should_hook_bypass(foreground_pid) {
            self.controller.reset();
            return (false, Vec::new());
        }

        let effects = self.controller.process_key(event, vk_code);
        let mut batches = Vec::new();
        let mut forward_original = false;

        for eff in effects {
            match eff {
                Effect::CommitText { text } => {
                    if !text.is_empty() {
                        batches.push(WinInputBatch::SendUnicodeText(text));
                    }
                }
                Effect::ReplaceText { delete_utf16, insert, .. } => {
                    if delete_utf16 > 0 {
                        batches.push(WinInputBatch::SendBackspaces(delete_utf16));
                    }
                    if !insert.is_empty() {
                        batches.push(WinInputBatch::SendUnicodeText(insert));
                    }
                }
                Effect::ForwardKey { .. } => {
                    forward_original = true;
                }
                _ => {}
            }
        }

        let consume_physical_key = !batches.is_empty() && !forward_original;
        (consume_physical_key, batches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_bypasses_when_tsf_is_active_for_same_pid() {
        let routing = SharedRoutingBlock::new();
        let mut hook = WindowsHookProcessor::new(Config::default());

        // Khi TSF không hoạt động ở PID 500 (ví dụ Windows Terminal), Hook xử lý bình thường
        let _ = hook.process_hook_key(&routing, 500, KeyEvent::ch('a'), 0x41);
        let (consume, batches) = hook.process_hook_key(&routing, 500, KeyEvent::ch('s'), 0x53);
        assert!(consume);
        assert_eq!(
            batches,
            vec![WinInputBatch::SendBackspaces(1), WinInputBatch::SendUnicodeText("á".into())]
        );

        // Khi TSF công bố đang hoạt động ở PID 600 (Notepad), Hook tự động nhường
        routing.publish_tsf_active(600, 0x1234);
        let (consume_tsf, batches_tsf) = hook.process_hook_key(&routing, 600, KeyEvent::ch('a'), 0x41);
        assert!(!consume_tsf);
        assert!(batches_tsf.is_empty());
    }
}
