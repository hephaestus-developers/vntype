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

#[cfg(windows)]
pub mod win_edit_session {
    use core::mem::ManuallyDrop;
    use std::cell::RefCell;
    use std::rc::Rc;
    use windows::Win32::UI::TextServices::{
        ITfComposition, ITfCompositionSink, ITfContext, ITfContextComposition, ITfEditSession,
        ITfEditSession_Impl, ITfInsertAtSelection, ITfRange, TF_AE_NONE, TF_ANCHOR_END, TF_SELECTION,
        TF_SELECTIONSTYLE, TfEditCookie,
    };
    use windows_core::{Interface, implement};

    use vntype_adapter::Effect;

    use crate::composition::CompositionTracker;

    use super::{TsfContextBackend, apply_effects_in_edit_session};

    /// Triển khai [`TsfContextBackend`] trên `ITfContext` thật của Windows TSF.
    pub struct WinTsfContextBackend<'a> {
        pub context: &'a ITfContext,
        pub ec: TfEditCookie,
        pub composition: &'a mut Option<ITfComposition>,
        pub comp_sink: &'a ITfCompositionSink,
        pub tracker: &'a mut CompositionTracker,
    }

    impl<'a> WinTsfContextBackend<'a> {
        fn current_selection_range(&self) -> Option<ITfRange> {
            unsafe {
                let mut sel =
                    [TF_SELECTION { range: ManuallyDrop::new(None), style: TF_SELECTIONSTYLE::default() }];
                let mut fetched = 0u32;
                if self.context.GetSelection(self.ec, 0, &mut sel, &mut fetched).is_err() || fetched == 0 {
                    return None;
                }
                ManuallyDrop::into_inner(core::ptr::read(&sel[0].range))
            }
        }

        fn set_caret_to_range_end(&self, range: &ITfRange) {
            unsafe {
                let Ok(caret) = range.Clone() else {
                    return;
                };
                if caret.Collapse(self.ec, TF_ANCHOR_END).is_ok() {
                    let sel = [TF_SELECTION {
                        range: ManuallyDrop::new(Some(caret)),
                        style: TF_SELECTIONSTYLE { ase: TF_AE_NONE, fInterimChar: false.into() },
                    }];
                    let _ = self.context.SetSelection(self.ec, &sel);
                }
            }
        }
    }

    impl<'a> TsfContextBackend for WinTsfContextBackend<'a> {
        fn set_composition_text(&mut self, text: &str, cursor_utf16: u32) {
            let utf16: Vec<u16> = text.encode_utf16().collect();
            unsafe {
                if self.composition.is_none() {
                    if let (Ok(ctx_comp), Some(sel_range)) =
                        (self.context.cast::<ITfContextComposition>(), self.current_selection_range())
                    {
                        if let Ok(comp) = ctx_comp.StartComposition(self.ec, &sel_range, self.comp_sink) {
                            *self.composition = Some(comp);
                        }
                    }
                }

                if let Some(comp) = self.composition.as_ref() {
                    if let Ok(range) = comp.GetRange() {
                        let _ = range.SetText(self.ec, 0, &utf16);
                        if let Ok(caret) = range.Clone() {
                            let _ = caret.Collapse(self.ec, TF_ANCHOR_END);
                            let back = utf16.len().saturating_sub(cursor_utf16 as usize) as i32;
                            if back > 0 {
                                let mut shifted = 0i32;
                                let _ = caret.ShiftStart(self.ec, -back, &mut shifted, core::ptr::null());
                                let _ = caret.Collapse(self.ec, TF_ANCHOR_END);
                            }
                            let sel = [TF_SELECTION {
                                range: ManuallyDrop::new(Some(caret)),
                                style: TF_SELECTIONSTYLE { ase: TF_AE_NONE, fInterimChar: false.into() },
                            }];
                            let _ = self.context.SetSelection(self.ec, &sel);
                        }
                    }
                }
            }
        }

        fn end_composition_at_end(&mut self) {
            let Some(comp) = self.composition.take() else {
                return;
            };
            self.tracker.ending_internally = true;
            unsafe {
                let end_range = comp.GetRange().ok().and_then(|r| r.Clone().ok());
                let _ = comp.EndComposition(self.ec);
                if let Some(r) = end_range {
                    self.set_caret_to_range_end(&r);
                }
            }
            self.tracker.ending_internally = false;
        }

        fn commit_text_at_selection(&mut self, text: &str) {
            let utf16: Vec<u16> = text.encode_utf16().collect();
            if let Some(comp) = self.composition.take() {
                self.tracker.ending_internally = true;
                unsafe {
                    if let Ok(range) = comp.GetRange() {
                        let _ = range.SetText(self.ec, 0, &utf16);
                        let end_range = range.Clone().ok();
                        let _ = comp.EndComposition(self.ec);
                        if let Some(r) = end_range {
                            self.set_caret_to_range_end(&r);
                        }
                    } else {
                        let _ = comp.EndComposition(self.ec);
                    }
                }
                self.tracker.ending_internally = false;
                return;
            }

            unsafe {
                if let Ok(insert_at_sel) = self.context.cast::<ITfInsertAtSelection>() {
                    if let Ok(range) = insert_at_sel.InsertTextAtSelection(self.ec, 0, &utf16) {
                        self.set_caret_to_range_end(&range);
                    }
                }
            }
        }

        fn replace_before_caret(&mut self, delete_utf16: u32, insert: &str) {
            let utf16: Vec<u16> = insert.encode_utf16().collect();
            unsafe {
                let Some(range) = self.current_selection_range() else {
                    return;
                };
                if delete_utf16 > 0 {
                    let mut shifted = 0i32;
                    let _ =
                        range.ShiftStart(self.ec, -(delete_utf16 as i32), &mut shifted, core::ptr::null());
                }
                let _ = range.SetText(self.ec, 0, &utf16);
                self.set_caret_to_range_end(&range);
            }
        }
    }

    /// Phiên chỉnh sửa đồng bộ `ITfEditSession` thực thi danh sách [`Effect`] lên `ITfContext`.
    #[implement(ITfEditSession)]
    pub struct WinEditSession {
        context: ITfContext,
        effects: Vec<Effect>,
        tracker: Rc<RefCell<CompositionTracker>>,
        composition: Rc<RefCell<Option<ITfComposition>>>,
        comp_sink: ITfCompositionSink,
        eaten: Rc<RefCell<bool>>,
    }

    impl WinEditSession {
        pub fn new(
            context: ITfContext,
            effects: Vec<Effect>,
            tracker: Rc<RefCell<CompositionTracker>>,
            composition: Rc<RefCell<Option<ITfComposition>>>,
            comp_sink: ITfCompositionSink,
            eaten: Rc<RefCell<bool>>,
        ) -> Self {
            Self { context, effects, tracker, composition, comp_sink, eaten }
        }
    }

    impl ITfEditSession_Impl for WinEditSession_Impl {
        fn DoEditSession(&self, ec: TfEditCookie) -> windows_core::Result<()> {
            let mut tracker = self.tracker.borrow_mut();
            let mut comp = self.composition.borrow_mut();
            let mut dummy_tracker = tracker.clone();
            let mut backend = WinTsfContextBackend {
                context: &self.context,
                ec,
                composition: &mut comp,
                comp_sink: &self.comp_sink,
                tracker: &mut dummy_tracker,
            };
            let result = apply_effects_in_edit_session(&self.effects, &mut tracker, &mut backend);
            *self.eaten.borrow_mut() = result;
            Ok(())
        }
    }
}
