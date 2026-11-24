//! Máy trạng thái điều phối trung tâm của Adapter (`AdapterController`) và đường ống hiệu ứng nguyên tử ([`Effect`]).
//!
//! Thay vì để từng hệ điều hành (Linux Fcitx5, IBus, Wayland, Windows TSF, macOS IMK) tự viết lại
//! logic xử lý `FailSafe` hoặc hàng đợi giao dịch bất đồng bộ, [`AdapterController`] đóng gói hoàn toàn:
//! 1. Tự động chốt chữ đang soạn (`CommitText` + `ClearPreedit`) trước khi `reset()` khi gặp `FailSafe`
//!    ở chế độ `Preedit`, chấm dứt hoàn toàn lỗi mất chữ trên macOS và sự thiếu nhất quán giữa các OS.
//! 2. Cung cấp hàng đợi giao dịch (`queue`) khi nền tảng yêu cầu thực thi `Replace` bất đồng bộ có hẹn giờ.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use vntype_engine::{
    Action, AppContext, Config, ConfigStore, Dictionary, FieldKind, ImeSession, KeyEvent, OutputMode,
    Ports, SurroundingText, WordListDictionary, shared_dictionary,
};

use crate::field_policy;
use crate::offset::{char_to_byte_offset, is_word_codepoint, utf16_len, utf16_prefix};
use crate::planner::{
    AdapterAction, POST_COMMIT_DELAY_MS, PlanContext, PlanKind, ReplaceFallback, plan_action_with_fallback,
};
use crate::surrounding_guard::SurroundingSnapshot;

/// Một hiệu ứng nguyên tử mà lớp vỏ hệ điều hành cần thực thi theo đúng thứ tự.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Hiển thị chuỗi văn bản gạch chân tạm thời (Preedit / Marked Text).
    SetPreedit {
        /// Nội dung chuỗi gạch chân (UTF-8).
        text: String,
        /// Vị trí con trỏ tính theo đơn vị mã UTF-16.
        cursor_utf16: u32,
        /// Vị trí con trỏ tính theo số byte UTF-8.
        cursor_bytes: u32,
    },
    /// Xoá vùng văn bản gạch chân tạm thời.
    ClearPreedit,
    /// Chốt một chuỗi văn bản chính thức vào ứng dụng đích.
    CommitText {
        /// Nội dung văn bản cần chốt (UTF-8).
        text: String,
    },
    /// Xoá lùi một đoạn văn bản ngay trước con trỏ rồi chèn văn bản mới (hoặc chỉ xoá nếu chạy bất đồng bộ).
    ReplaceText {
        /// Số đơn vị mã UTF-16 cần xoá lùi.
        delete_utf16: u32,
        /// Số ký tự Unicode (`char`) cần xoá lùi.
        delete_chars: u32,
        /// Số byte UTF-8 cần xoá lùi.
        delete_bytes: u32,
        /// Chuỗi văn bản cần chèn vào sau khi xoá.
        insert: String,
    },
    /// Yêu cầu bộ điều phối sự kiện của hệ điều hành hẹn giờ chờ `ms` mili-giây rồi gọi `on_timer_tick`.
    WaitMs {
        /// Số mili-giây cần chờ.
        ms: u32,
    },
    /// Chuyển tiếp sự kiện phím gốc (có thể kèm `raw_keycode` của phím đã xếp hàng) về cho ứng dụng.
    ForwardKey {
        /// Mã phím vật lý hoặc định danh sự kiện của hệ điều hành (`0` nếu là phím đang xử lý đồng bộ hiện tại).
        raw_keycode: u32,
    },
}

/// Một sự kiện phím được giữ trong hàng đợi trong lúc giao dịch `Replace` bất đồng bộ đang chờ ứng dụng.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedKey {
    /// Sự kiện phím chuẩn hoá.
    pub event: KeyEvent,
    /// Mã phím vật lý gốc của hệ điều hành để chuyển tiếp khi cần.
    pub raw_keycode: u32,
}

#[derive(Clone)]
struct SharedState {
    config: Config,
    app: Option<String>,
    surrounding_before: Option<String>,
    dictionary: Arc<WordListDictionary>,
    config_version: u64,
}

#[derive(Clone)]
struct ControllerPorts(Rc<RefCell<SharedState>>);

impl ConfigStore for ControllerPorts {
    fn load(&self) -> Config {
        self.0.borrow().config.clone()
    }
    fn save(&self, config: &Config) {
        let mut s = self.0.borrow_mut();
        s.config = config.clone();
        s.config_version += 1;
    }
}

impl AppContext for ControllerPorts {
    fn current_app(&self) -> Option<String> {
        self.0.borrow().app.clone()
    }
}

impl SurroundingText for ControllerPorts {
    fn before_cursor(&self, n: usize) -> Option<String> {
        let s = self.0.borrow();
        let t = s.surrounding_before.as_ref()?;
        if n == 0 {
            return Some(String::new());
        }
        let slice = match t.char_indices().rev().nth(n - 1) {
            Some((idx, _)) => &t[idx..],
            None => t.as_str(),
        };
        Some(slice.to_string())
    }
}

impl Dictionary for ControllerPorts {
    fn is_english(&self, word: &str) -> bool {
        self.0.borrow().dictionary.english.contains(word)
    }
    fn is_vietnamese(&self, word: &str) -> bool {
        self.0.borrow().dictionary.vietnamese.contains(word)
    }
}

#[derive(Debug, Clone)]
struct FullSurrounding {
    text: String,
    cursor_utf16: u32,
    anchor_utf16: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AsyncPhase {
    Idle,
    WaitingReplaceCommit { text: String, pass_key: bool, raw_keycode: u32 },
    WaitingPostCommitDrain,
}

/// Bộ điều khiển trung tâm cho một ngữ cảnh nhập liệu trên bất kỳ hệ điều hành nào.
///
/// # Examples
///
/// ```
/// use vntype_adapter::{AdapterController, Effect};
/// use vntype_engine::{Config, KeyEvent, OutputMode};
///
/// let mut ctrl = AdapterController::new(Config::default(), OutputMode::Preedit);
/// let effects = ctrl.process_key(KeyEvent::ch('a'), 1);
/// assert_eq!(effects, vec![Effect::SetPreedit { text: "a".into(), cursor_utf16: 1, cursor_bytes: 1 }]);
/// ```
pub struct AdapterController {
    session: ImeSession,
    shared: ControllerPorts,
    surrounding: Option<FullSurrounding>,
    allow_unverified_replace: bool,
    async_replace: bool,
    phase: AsyncPhase,
    queue: VecDeque<QueuedKey>,
}

impl AdapterController {
    /// Khởi tạo [`AdapterController`] với cấu hình ban đầu và chế độ xuất chữ `mode`.
    pub fn new(config: Config, mode: OutputMode) -> Self {
        let shared = ControllerPorts(Rc::new(RefCell::new(SharedState {
            config,
            app: None,
            surrounding_before: None,
            dictionary: shared_dictionary(),
            config_version: 0,
        })));
        let ports = Ports {
            config: Box::new(shared.clone()),
            app: Box::new(shared.clone()),
            surrounding: Box::new(shared.clone()),
            dictionary: Box::new(shared.clone()),
        };
        AdapterController {
            session: ImeSession::new(ports, mode),
            shared,
            surrounding: None,
            allow_unverified_replace: false,
            async_replace: false,
            phase: AsyncPhase::Idle,
            queue: VecDeque::new(),
        }
    }

    /// Bật hoặc tắt chế độ cho phép `Replace` trực tiếp không cần xác minh surrounding (dành cho Windows Hook).
    pub fn set_allow_unverified_replace(&mut self, allow: bool) {
        self.allow_unverified_replace = allow;
    }

    /// Bật hoặc tắt chế độ tách `Replace` thành giao dịch bất đồng bộ có `Effect::WaitMs` và hàng đợi phím.
    pub fn set_async_replace(&mut self, enabled: bool) {
        self.async_replace = enabled;
    }

    /// Kiểm tra xem controller có đang bận trong một giao dịch `Replace` bất đồng bộ hay không.
    pub fn is_busy(&self) -> bool {
        self.phase != AsyncPhase::Idle
    }

    /// Trả về tham chiếu tới [`ImeSession`] bên trong.
    pub fn session(&self) -> &ImeSession {
        &self.session
    }

    /// Trả về chuỗi văn bản mà engine tin là đang hiển thị cho từ hiện tại.
    pub fn display_text(&self) -> String {
        self.session.display()
    }

    /// Kiểm tra bộ gõ tiếng Việt có đang bật hay không.
    pub fn is_enabled(&self) -> bool {
        self.session.is_enabled()
    }

    /// Trả về số phiên bản cấu hình nội bộ (tăng mỗi khi người dùng bấm `Alt+,` lưu trạng thái theo ứng dụng).
    pub fn config_version(&self) -> u64 {
        self.shared.0.borrow().config_version
    }

    /// Bổ sung các từ mới vào từ điển của phiên hiện tại theo cơ chế Copy-on-Write.
    pub fn add_dictionary_words(&mut self, words: &str, vietnamese: bool) {
        let mut sh = self.shared.0.borrow_mut();
        let dict = Arc::make_mut(&mut sh.dictionary);
        dict.add_words(words.split_whitespace(), vietnamese);
    }

    /// Nạp mới danh sách từ điển tiếng Anh hoặc tiếng Việt cho phiên hiện tại.
    pub fn load_dictionary(&mut self, english: Option<&str>, vietnamese: Option<&str>) {
        if english.is_none() && vietnamese.is_none() {
            return;
        }
        let mut sh = self.shared.0.borrow_mut();
        let dict = Arc::make_mut(&mut sh.dictionary);
        if let Some(en) = english {
            dict.english = en.split_whitespace().map(|w| w.to_lowercase()).collect();
        }
        if let Some(vi) = vietnamese {
            dict.vietnamese = vi.split_whitespace().map(|w| w.to_lowercase()).collect();
        }
    }

    /// Cập nhật ảnh chụp nhanh văn bản xung quanh con trỏ (theo toạ độ đơn vị mã UTF-16).
    ///
    /// Nếu đang có vùng chọn (`cursor_utf16 != anchor_utf16`) hoặc con trỏ đang đứng giữa từ,
    /// hàm tự động ẩn `surrounding_before` khỏi engine để ngăn việc nạp nhầm một tiền tố cụt.
    pub fn set_surrounding_utf16(&mut self, text: Option<&str>, cursor_utf16: u32, anchor_utf16: u32) {
        let Some(text) = text else {
            self.surrounding = None;
            self.shared.0.borrow_mut().surrounding_before = None;
            return;
        };
        let Some(before_slice) = utf16_prefix(text, cursor_utf16) else {
            self.surrounding = None;
            self.shared.0.borrow_mut().surrounding_before = None;
            return;
        };
        self.surrounding = Some(FullSurrounding { text: text.to_string(), cursor_utf16, anchor_utf16 });

        if cursor_utf16 != anchor_utf16 {
            self.shared.0.borrow_mut().surrounding_before = None;
            return;
        }
        let after_slice = &text[before_slice.len()..];
        if after_slice.chars().next().is_some_and(is_word_codepoint) {
            self.shared.0.borrow_mut().surrounding_before = None;
            return;
        }
        self.shared.0.borrow_mut().surrounding_before = Some(before_slice.to_string());
    }

    /// Cập nhật văn bản ngay trước con trỏ khi nền tảng chỉ cung cấp chuỗi phía trước con trỏ.
    pub fn set_surrounding_before_only(&mut self, before_cursor: Option<&str>) {
        match before_cursor {
            Some(before) => {
                let len_u16 = utf16_len(before);
                self.surrounding = Some(FullSurrounding {
                    text: before.to_string(),
                    cursor_utf16: len_u16,
                    anchor_utf16: len_u16,
                });
                self.shared.0.borrow_mut().surrounding_before = Some(before.to_string());
            }
            None => {
                self.surrounding = None;
                self.shared.0.borrow_mut().surrounding_before = None;
            }
        }
    }

    /// Xử lý một sự kiện phím và trả về danh sách các hiệu ứng nguyên tử [`Effect`].
    pub fn process_key(&mut self, event: KeyEvent, raw_keycode: u32) -> Vec<Effect> {
        if self.is_busy() {
            self.queue.push_back(QueuedKey { event, raw_keycode });
            return Vec::new();
        }
        self.process_key_immediate(&event, raw_keycode)
    }

    fn process_key_immediate(&mut self, event: &KeyEvent, raw_keycode: u32) -> Vec<Effect> {
        // Phím nóng chuyển Việt/Anh theo cấu hình `switch_key`
        if event.alt && !event.ctrl && !event.meta {
            let matched = match self.session.config().switch_key {
                vntype_engine::SwitchKey::AltComma => event.key == vntype_engine::Key::Char(','),
                vntype_engine::SwitchKey::AltZ => {
                    matches!(event.key, vntype_engine::Key::Char('z' | 'Z'))
                }
                vntype_engine::SwitchKey::CtrlShift => false,
            };
            if matched {
                return self.toggle_vietnamese();
            }
        }

        let display = self.session.display();
        let before = if !display.is_empty() {
            display
        } else {
            self.shared.0.borrow().surrounding_before.clone().unwrap_or_default()
        };
        let was_adopted = self.session.is_adopted();
        let act = self.session.process_key(event);
        let effects = self.translate_action(act, &before, was_adopted, Some(raw_keycode));
        if matches!(event.key, vntype_engine::Key::Enter | vntype_engine::Key::Tab) || event.key.is_nav() {
            self.set_surrounding_utf16(None, 0, 0);
        }
        effects
    }

    /// Khi bộ hẹn giờ của hệ điều hành điểm (`Effect::WaitMs` hoàn tất), hàm này hoàn tất giao dịch
    /// và xả tiếp các phím đang nằm trong hàng đợi.
    pub fn on_timer_tick(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        match std::mem::replace(&mut self.phase, AsyncPhase::Idle) {
            AsyncPhase::Idle => {}
            AsyncPhase::WaitingReplaceCommit { text, pass_key, raw_keycode } => {
                if !text.is_empty() {
                    effects.push(Effect::CommitText { text: text.clone() });
                    self.update_surrounding_after_commit(&text);
                }
                effects.push(Effect::ClearPreedit);
                if pass_key {
                    effects.push(Effect::ForwardKey { raw_keycode });
                }
                if !self.queue.is_empty() {
                    self.phase = AsyncPhase::WaitingPostCommitDrain;
                    effects.push(Effect::WaitMs { ms: POST_COMMIT_DELAY_MS });
                }
            }
            AsyncPhase::WaitingPostCommitDrain => {
                while !self.is_busy() {
                    let Some(queued) = self.queue.pop_front() else { break };
                    let more = self.process_key_immediate(&queued.event, queued.raw_keycode);
                    effects.extend(more);
                }
            }
        }
        effects
    }

    /// Thông báo ứng dụng nhận tiêu điểm thay đổi (`app_id`).
    pub fn focus_changed(&mut self, app_id: Option<&str>) -> Vec<Effect> {
        let mut effects = self.cancel_pending_async();
        let before = self.session.display();
        let was_adopted = self.session.is_adopted();
        self.shared.0.borrow_mut().app = app_id.map(str::to_string);
        let act = self.session.focus_changed(app_id);
        effects.extend(self.translate_action(act, &before, was_adopted, None));
        effects
    }

    /// Thông báo loại trường nhập liệu thay đổi.
    ///
    /// Nếu chuyển sang trường nhạy cảm/bị tắt (`Password`, `Number`, `Email`), vùng văn bản đang soạn dở
    /// sẽ bị xoá sạch mà không chốt vào trường mới.
    pub fn set_field_kind(&mut self, kind: FieldKind) -> Vec<Effect> {
        let was_disabled = field_policy::is_disabled(self.session.field_kind());
        let now_disabled = field_policy::is_disabled(kind);
        if now_disabled && !was_disabled {
            // Huỷ ngay hàng đợi và trạng thái đang soạn mà không chốt chữ vào ô nhạy cảm.
            self.reset();
            let _ = self.session.set_field_kind(kind);
            return vec![Effect::ClearPreedit];
        }

        let mut effects = self.cancel_pending_async();
        let before = self.session.display();
        let was_adopted = self.session.is_adopted();
        let act = self.session.set_field_kind(kind);
        effects.extend(self.translate_action(act, &before, was_adopted, None));
        effects
    }

    /// Thông báo con trỏ bị di chuyển ra chỗ khác.
    pub fn cursor_moved(&mut self) -> Vec<Effect> {
        let mut effects = self.cancel_pending_async();
        let before = self.session.display();
        let was_adopted = self.session.is_adopted();
        let act = self.session.cursor_moved();
        effects.extend(self.translate_action(act, &before, was_adopted, None));
        effects
    }

    /// Đảo trạng thái bật/tắt tiếng Việt.
    pub fn toggle_vietnamese(&mut self) -> Vec<Effect> {
        let mut effects = self.cancel_pending_async();
        let before = self.session.display();
        let was_adopted = self.session.is_adopted();
        let act = self.session.toggle_vietnamese();
        effects.extend(self.translate_action(act, &before, was_adopted, None));
        effects
    }

    /// Chốt vùng văn bản đang soạn dở trước khi đóng hoặc mất tiêu điểm.
    pub fn flush(&mut self) -> Vec<Effect> {
        let mut effects = self.cancel_pending_async();
        let before = self.session.display();
        let was_adopted = self.session.is_adopted();
        let act = self.session.flush_composition();
        effects.extend(self.translate_action(act, &before, was_adopted, None));
        effects
    }

    /// Thay đổi chế độ xuất chữ (`Preedit` hoặc `Replace`).
    pub fn set_output_mode(&mut self, mode: OutputMode) -> Vec<Effect> {
        let mut effects = self.cancel_pending_async();
        let before = self.session.display();
        let was_adopted = self.session.is_adopted();
        let act = self.session.set_output_mode(mode);
        effects.extend(self.translate_action(act, &before, was_adopted, None));
        effects
    }

    /// Áp cấu hình mới trực tiếp lên phiên hiện tại và trả về các hiệu ứng cần thực thi trước khi gõ tiếp.
    pub fn apply_config(&mut self, config: Config) -> Vec<Effect> {
        let mut effects = self.cancel_pending_async();
        let before = self.session.display();
        let was_adopted = self.session.is_adopted();
        self.shared.0.borrow_mut().config = config;
        let act = self.session.reload_config();
        effects.extend(self.translate_action(act, &before, was_adopted, None));
        effects
    }

    /// Áp một mảnh JSON cấu hình mới lên phiên hiện tại và trả về các hiệu ứng cần thực thi trước khi gõ tiếp.
    pub fn apply_config_json(&mut self, json_patch: &str) -> Option<Vec<Effect>> {
        let base = self.shared.0.borrow().config.clone();
        let patched = base.apply_json_patch(json_patch)?;
        Some(self.apply_config(patched))
    }

    /// Xoá sạch trạng thái phiên gõ và hàng đợi mà không chốt văn bản.
    pub fn reset(&mut self) {
        self.phase = AsyncPhase::Idle;
        self.queue.clear();
        self.session.reset();
    }

    fn cancel_pending_async(&mut self) -> Vec<Effect> {
        self.queue.clear();
        let mut effects = Vec::new();
        if let AsyncPhase::WaitingReplaceCommit { text, pass_key, raw_keycode } =
            std::mem::replace(&mut self.phase, AsyncPhase::Idle)
        {
            if !text.is_empty() {
                effects.push(Effect::CommitText { text: text.clone() });
                self.update_surrounding_after_commit(&text);
            }
            effects.push(Effect::ClearPreedit);
            if pass_key {
                effects.push(Effect::ForwardKey { raw_keycode });
            }
        }
        effects
    }

    fn translate_action(
        &mut self,
        action: Action,
        before_display: &str,
        was_adopted: bool,
        raw_keycode: Option<u32>,
    ) -> Vec<Effect> {
        let adapter_action = AdapterAction::from(action);
        let snapshot = self.surrounding.as_ref().map(|s| SurroundingSnapshot {
            text: &s.text,
            cursor_utf16: s.cursor_utf16,
            anchor_utf16: s.anchor_utf16,
        });
        let context = PlanContext {
            field_kind: self.session.field_kind(),
            before_display,
            surrounding: snapshot,
            allow_unverified_replace: self.allow_unverified_replace,
        };
        let fallback = if self.allow_unverified_replace || was_adopted {
            ReplaceFallback::Always
        } else if snapshot.is_some() {
            ReplaceFallback::StaleSurrounding
        } else {
            ReplaceFallback::None
        };
        let plan = plan_action_with_fallback(&adapter_action, &context, self.session.profile(), fallback);

        let mut effects = Vec::new();
        match plan.kind {
            PlanKind::Pass => {
                if field_policy::is_disabled(self.session.field_kind()) {
                    effects.push(Effect::ClearPreedit);
                }
                if let Some(code) = raw_keycode {
                    effects.push(Effect::ForwardKey { raw_keycode: code });
                }
            }
            PlanKind::Preedit => {
                if plan.text.is_empty() {
                    effects.push(Effect::ClearPreedit);
                } else {
                    effects.push(Effect::SetPreedit {
                        text: plan.text,
                        cursor_utf16: plan.cursor_utf16,
                        cursor_bytes: plan.cursor_bytes,
                    });
                }
            }
            PlanKind::Commit => {
                if !plan.text.is_empty() {
                    effects.push(Effect::CommitText { text: plan.text.clone() });
                    self.update_surrounding_after_commit(&plan.text);
                }
                effects.push(Effect::ClearPreedit);
                if plan.pass_key {
                    self.set_surrounding_utf16(None, 0, 0);
                    if let Some(code) = raw_keycode {
                        effects.push(Effect::ForwardKey { raw_keycode: code });
                    }
                }
            }
            PlanKind::Replace => {
                if plan.delete_utf16 == 0 {
                    if !plan.text.is_empty() {
                        effects.push(Effect::CommitText { text: plan.text.clone() });
                        self.update_surrounding_after_commit(&plan.text);
                    }
                    effects.push(Effect::ClearPreedit);
                    if plan.pass_key {
                        self.set_surrounding_utf16(None, 0, 0);
                        if let Some(code) = raw_keycode {
                            effects.push(Effect::ForwardKey { raw_keycode: code });
                        }
                    }
                } else if self.async_replace && plan.wait_ms > 0 {
                    // Giao dịch bất đồng bộ 2 pha: Xoá trước -> Chờ wait_ms -> Commit sau
                    effects.push(Effect::ReplaceText {
                        delete_utf16: plan.delete_utf16,
                        delete_chars: plan.delete_chars,
                        delete_bytes: plan.delete_bytes,
                        insert: String::new(),
                    });
                    self.update_surrounding_after_delete(plan.delete_chars);
                    self.phase = AsyncPhase::WaitingReplaceCommit {
                        text: plan.text,
                        pass_key: plan.pass_key,
                        raw_keycode: raw_keycode.unwrap_or(0),
                    };
                    effects.push(Effect::WaitMs { ms: plan.wait_ms });
                } else {
                    effects.push(Effect::ClearPreedit);
                    effects.push(Effect::ReplaceText {
                        delete_utf16: plan.delete_utf16,
                        delete_chars: plan.delete_chars,
                        delete_bytes: plan.delete_bytes,
                        insert: plan.text.clone(),
                    });
                    self.update_surrounding_after_delete(plan.delete_chars);
                    self.update_surrounding_after_commit(&plan.text);
                    if plan.pass_key {
                        self.set_surrounding_utf16(None, 0, 0);
                        if let Some(code) = raw_keycode {
                            effects.push(Effect::ForwardKey { raw_keycode: code });
                        }
                    }
                }
            }
            PlanKind::FailSafe => {
                // Xử lý Fail-Safe chuẩn hoá cho MỌI hệ điều hành:
                // Nếu đang ở chế độ Preedit và chữ chưa từng được commit lên màn hình (!was_adopted),
                // ta BẮT BUỘC phải CommitText(before_display) trước khi xoá preedit để không bao giờ mất chữ!
                if self.session.mode() == OutputMode::Preedit && !was_adopted && !before_display.is_empty()
                {
                    effects.push(Effect::CommitText { text: before_display.to_string() });
                    self.update_surrounding_after_commit(before_display);
                }
                self.session.reset();
                effects.push(Effect::ClearPreedit);
                if let Some(code) = raw_keycode {
                    effects.push(Effect::ForwardKey { raw_keycode: code });
                }
            }
        }
        effects
    }

    fn update_surrounding_after_delete(&mut self, delete_chars: u32) {
        let Some(sur) = self.surrounding.as_mut() else { return };
        if delete_chars == 0 {
            return;
        }
        let Some(before) = utf16_prefix(&sur.text, sur.cursor_utf16) else {
            self.surrounding = None;
            return;
        };
        let total_chars = before.chars().count() as u32;
        if delete_chars > total_chars {
            self.surrounding = None;
            return;
        }
        let keep_chars = total_chars - delete_chars;
        let Some(keep_byte) = char_to_byte_offset(before, keep_chars) else {
            self.surrounding = None;
            return;
        };
        let after_cursor_byte = before.len();
        let mut updated = sur.text[..keep_byte].to_string();
        updated.push_str(&sur.text[after_cursor_byte..]);
        let new_cursor_u16 = utf16_len(&updated[..keep_byte]);
        sur.text = updated;
        sur.cursor_utf16 = new_cursor_u16;
        sur.anchor_utf16 = new_cursor_u16;
        self.shared.0.borrow_mut().surrounding_before = Some(sur.text[..keep_byte].to_string());
    }

    fn update_surrounding_after_commit(&mut self, insert: &str) {
        if insert.is_empty() {
            return;
        }
        let Some(sur) = self.surrounding.as_mut() else { return };
        let Some(before) = utf16_prefix(&sur.text, sur.cursor_utf16) else {
            self.surrounding = None;
            return;
        };
        let byte_idx = before.len();
        let mut updated = sur.text[..byte_idx].to_string();
        updated.push_str(insert);
        updated.push_str(&sur.text[byte_idx..]);
        let new_byte_idx = byte_idx + insert.len();
        let new_cursor_u16 = utf16_len(&updated[..new_byte_idx]);
        sur.text = updated;
        sur.cursor_utf16 = new_cursor_u16;
        sur.anchor_utf16 = new_cursor_u16;
        self.shared.0.borrow_mut().surrounding_before = Some(sur.text[..new_byte_idx].to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vntype_engine::Key;

    #[test]
    fn preedit_then_space_emits_commit_and_clear() {
        let mut ctrl = AdapterController::new(Config::default(), OutputMode::Preedit);
        for (i, ch) in "tieengs".chars().enumerate() {
            let effects = ctrl.process_key(KeyEvent::ch(ch), i as u32 + 1);
            assert!(effects.iter().any(|e| matches!(e, Effect::SetPreedit { .. })));
        }
        assert_eq!(ctrl.display_text(), "tiếng");
        let effects = ctrl.process_key(KeyEvent::ch(' '), 57);
        assert_eq!(effects, vec![Effect::CommitText { text: "tiếng ".into() }, Effect::ClearPreedit]);
    }

    #[test]
    fn fail_safe_in_preedit_preserves_word_before_reset() {
        let mut ctrl = AdapterController::new(Config::default(), OutputMode::Preedit);
        ctrl.process_key(KeyEvent::ch('v'), 1);
        ctrl.process_key(KeyEvent::ch('i'), 2);
        // Giả lập một tình huống Replace bị FailSafe (đang có vùng bôi đen mở)
        ctrl.set_surrounding_utf16(Some("vi"), 2, 0);
        let effects = ctrl.translate_action(
            Action::Replace { delete_before: 1, insert: "ỉ".into(), pass_key: false },
            "vi",
            false,
            Some(99),
        );
        // Phải chốt "vi", xoá preedit rồi mới chuyển tiếp phím 99
        assert_eq!(
            effects,
            vec![
                Effect::CommitText { text: "vi".into() },
                Effect::ClearPreedit,
                Effect::ForwardKey { raw_keycode: 99 }
            ]
        );
        assert!(ctrl.display_text().is_empty());
    }

    #[test]
    fn async_transaction_queues_incoming_keys() {
        let mut ctrl = AdapterController::new(Config::default(), OutputMode::Preedit);
        ctrl.set_async_replace(true);
        ctrl.set_surrounding_utf16(Some("Chào"), 4, 4);
        // Gõ 's' để sửa từ đã commit "Chào" -> "Cháo" (sinh Replace)
        let effects1 = ctrl.process_key(KeyEvent::ch('s'), 31);
        assert!(ctrl.is_busy());
        assert!(effects1.iter().any(|e| matches!(e, Effect::WaitMs { .. })));

        // Gõ tiếp dấu cách trong lúc đang chờ timer -> phải được xếp hàng
        let queued_effects = ctrl.process_key(KeyEvent::ch(' '), 57);
        assert!(queued_effects.is_empty());

        // Pha 1 timer hoàn tất: commit "áo" và đặt timer post-commit
        let effects2 = ctrl.on_timer_tick();
        assert!(effects2.iter().any(|e| matches!(e, Effect::CommitText { text } if text == "áo")));

        // Pha 2 timer hoàn tất: xả phím dấu cách đã xếp hàng
        let effects3 = ctrl.on_timer_tick();
        assert!(!ctrl.is_busy());
        assert!(effects3.iter().any(|e| matches!(e, Effect::CommitText { text } if text.ends_with(' '))));
    }

    #[test]
    fn switching_to_password_field_drops_composition_without_commit() {
        let mut ctrl = AdapterController::new(Config::default(), OutputMode::Preedit);
        ctrl.process_key(KeyEvent::ch('a'), 1);
        assert_eq!(ctrl.display_text(), "a");
        let effects = ctrl.set_field_kind(FieldKind::Password);
        assert_eq!(effects, vec![Effect::ClearPreedit]);
        assert!(ctrl.display_text().is_empty());
        // Gõ trong ô password phải chuyển thẳng phím
        let pass = ctrl.process_key(KeyEvent::new(Key::Char('s')), 2);
        assert_eq!(pass, vec![Effect::ClearPreedit, Effect::ForwardKey { raw_keycode: 2 }]);
    }

    #[test]
    fn switching_to_password_while_async_busy_drops_pending_commit() {
        let mut ctrl = AdapterController::new(Config::default(), OutputMode::Preedit);
        ctrl.set_async_replace(true);
        ctrl.set_surrounding_utf16(Some("Chào"), 4, 4);
        ctrl.process_key(KeyEvent::ch('s'), 31);
        assert!(ctrl.is_busy());

        let effects = ctrl.set_field_kind(FieldKind::Password);
        assert!(!ctrl.is_busy());
        assert_eq!(effects, vec![Effect::ClearPreedit]);
    }
}
