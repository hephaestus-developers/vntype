//! Cài đặt dịch vụ `ITfTextInputProcessorEx`, `ITfThreadMgrEventSink` và `ITfKeyEventSink`
//! bằng Rust thuần, đóng gói trong một tệp `vntype_tsf.dll` duy nhất.

use vntype_adapter::{AdapterController, utf16_len};
use vntype_engine::{Config, Key, KeyEvent, OutputMode};

use crate::composition::CompositionTracker;
use crate::edit_session::{TsfContextBackend, apply_effects_in_edit_session};
use crate::input_scope::resolve_field_kind_from_scopes;

/// Bộ dịch vụ văn bản TSF (Text Service) gắn với một luồng giao diện Windows (`ITfThreadMgr`).
pub struct VntypeTextService {
    controller: AdapterController,
    composition: CompositionTracker,
    active_app: String,
}

impl VntypeTextService {
    /// Khởi tạo một thực thể `VntypeTextService` với cấu hình `config`.
    pub fn new(config: Config) -> Self {
        VntypeTextService {
            controller: AdapterController::new(config, OutputMode::Preedit),
            composition: CompositionTracker::default(),
            active_app: String::new(),
        }
    }

    /// Xử lý khi cửa sổ hoặc tài liệu TSF nhận tiêu điểm (`OnSetFocus`).
    pub fn on_set_focus<B: TsfContextBackend>(
        &mut self,
        app_name: &str,
        input_scopes: &[i32],
        backend: &mut B,
    ) {
        self.active_app = app_name.to_string();
        let focus_effects = self.controller.focus_changed(Some(app_name));
        apply_effects_in_edit_session(&focus_effects, &mut self.composition, backend);

        let kind = resolve_field_kind_from_scopes(input_scopes, app_name);
        let field_effects = self.controller.set_field_kind(kind);
        apply_effects_in_edit_session(&field_effects, &mut self.composition, backend);
    }

    /// Xử lý khi cửa sổ TSF mất tiêu điểm (`OnKillFocus`), chốt vùng soạn thảo nếu còn.
    pub fn on_kill_focus<B: TsfContextBackend>(&mut self, backend: &mut B) {
        let effects = self.controller.flush();
        apply_effects_in_edit_session(&effects, &mut self.composition, backend);
    }

    /// Xử lý sự kiện nhấn phím `ITfKeyEventSink::OnKeyDown`.
    ///
    /// Trả về `true` (`pfEaten = TRUE`) nếu TSF đã tiêu thụ phím, `false` nếu trả phím cho ứng dụng.
    pub fn on_key_down<B: TsfContextBackend>(
        &mut self,
        key_event: KeyEvent,
        vk_code: u32,
        surrounding_before: Option<&str>,
        backend: &mut B,
    ) -> bool {
        if !self.composition.active {
            if let Some(before) = surrounding_before {
                let u16_len = utf16_len(before);
                self.controller.set_surrounding_utf16(Some(before), u16_len, u16_len);
            } else {
                self.controller.set_surrounding_utf16(None, 0, 0);
            }
        }
        let effects = self.controller.process_key(key_event, vk_code);
        apply_effects_in_edit_session(&effects, &mut self.composition, backend)
    }

    /// Xử lý khi ứng dụng bên ngoài chủ động đóng vùng composition (`OnCompositionTerminated`).
    pub fn on_composition_terminated(&mut self) {
        if self.composition.ending_internally {
            return;
        }
        self.composition.clear();
        self.controller.reset();
    }

    /// Ánh xạ mã phím ảo Windows (`VK_*`) và ký tự đã dịch qua `ToUnicode` sang [`Key`].
    pub fn map_virtual_key(vk: u16, unicode_char: Option<char>) -> Key {
        match vk {
            0x08 => Key::Backspace,
            0x0D => Key::Enter,
            0x09 => Key::Tab,
            0x1B => Key::Escape,
            0x25 => Key::Left,
            0x26 => Key::Up,
            0x27 => Key::Right,
            0x28 => Key::Down,
            0x24 => Key::Home,
            0x23 => Key::End,
            0x2E => Key::Delete,
            0x21 => Key::PageUp,
            0x22 => Key::PageDown,
            0x10..=0x14 | 0x5B..=0x5C | 0xA0..=0xA5 => Key::Modifier,
            _ => match unicode_char {
                Some(c) if !c.is_control() => Key::Char(c),
                _ => Key::Other,
            },
        }
    }
}

#[cfg(windows)]
pub mod win_tsf {
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;
    use std::sync::atomic::Ordering;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, GetKeyboardState, ToUnicode, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
    };
    use windows::Win32::UI::TextServices::{
        ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext, ITfDocumentMgr,
        ITfEditSession, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfSource,
        ITfTextInputProcessor_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl, ITfThreadMgr,
        ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl, TF_ES_READWRITE, TF_ES_SYNC,
    };
    use windows_core::{BOOL, IUnknownImpl, Interface, Ref, implement};

    use vntype_config::{default_config_path, load_config_or_default};
    use vntype_engine::KeyEvent;
    use vntype_win_ipc::win_shm::SharedRoutingMapping;

    use crate::composition::CompositionTracker;
    use crate::decrement_dll_ref;
    use crate::edit_session::win_edit_session::WinEditSession;
    use crate::increment_dll_ref;
    use crate::input_scope::resolve_field_kind_from_scopes;

    use super::VntypeTextService;

    fn current_process_info() -> (u32, String) {
        let pid = std::process::id();
        let mut buf = [0u16; 512];
        let len = unsafe { GetModuleFileNameW(None, &mut buf) };
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        let app = Path::new(&full).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        (pid, app)
    }

    fn current_modifiers() -> (bool, bool, bool) {
        unsafe {
            let ctrl = (GetKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0;
            let alt = (GetKeyState(VK_MENU.0 as i32) as u16) & 0x8000 != 0;
            let meta = ((GetKeyState(VK_LWIN.0 as i32) as u16) & 0x8000 != 0)
                || ((GetKeyState(VK_RWIN.0 as i32) as u16) & 0x8000 != 0);
            (ctrl, alt, meta)
        }
    }

    fn translate_tsf_key(wparam: WPARAM, lparam: LPARAM) -> KeyEvent {
        let vk = (wparam.0 & 0xFFFF) as u16;
        let scan_code = ((lparam.0 >> 16) & 0xFF) as u32;
        let (ctrl, alt, meta) = current_modifiers();
        let unicode_char = unsafe {
            let mut kb_state = [0u8; 256];
            let _ = GetKeyboardState(&mut kb_state);
            let mut out = [0u16; 4];
            let n = ToUnicode(vk as u32, scan_code, Some(&kb_state), &mut out, 0);
            if n == 1 { char::from_u32(out[0] as u32) } else { None }
        };
        let key = VntypeTextService::map_virtual_key(vk, unicode_char);
        KeyEvent { key, ctrl, alt, meta }
    }

    /// Thực thể COM triển khai đầy đủ `ITfTextInputProcessorEx` cho VNType trên Windows.
    #[implement(ITfTextInputProcessorEx, ITfThreadMgrEventSink, ITfKeyEventSink, ITfCompositionSink)]
    pub struct WinTextService {
        service: RefCell<VntypeTextService>,
        tracker: Rc<RefCell<CompositionTracker>>,
        composition: Rc<RefCell<Option<ITfComposition>>>,
        thread_mgr: RefCell<Option<ITfThreadMgr>>,
        client_id: RefCell<u32>,
        thread_mgr_cookie: RefCell<u32>,
        routing: Option<SharedRoutingMapping>,
        last_config_gen: RefCell<u64>,
    }

    impl WinTextService {
        pub fn new() -> Self {
            increment_dll_ref();
            let cfg = load_config_or_default(default_config_path());
            let routing = SharedRoutingMapping::open_or_create().ok();
            let cfg_gen =
                routing.as_ref().map(|r| r.config_generation.load(Ordering::Acquire)).unwrap_or(0);
            Self {
                service: RefCell::new(VntypeTextService::new(cfg)),
                tracker: Rc::new(RefCell::new(CompositionTracker::default())),
                composition: Rc::new(RefCell::new(None)),
                thread_mgr: RefCell::new(None),
                client_id: RefCell::new(0),
                thread_mgr_cookie: RefCell::new(0),
                routing,
                last_config_gen: RefCell::new(cfg_gen),
            }
        }

        fn maybe_reload_config(&self) {
            let Some(routing) = &self.routing else {
                return;
            };
            let cfg_gen = routing.config_generation.load(Ordering::Acquire);
            if cfg_gen != *self.last_config_gen.borrow() {
                let cfg = load_config_or_default(default_config_path());
                *self.service.borrow_mut() = VntypeTextService::new(cfg);
                *self.last_config_gen.borrow_mut() = cfg_gen;
            }
        }

        fn run_edit_session(
            &self,
            context: &ITfContext,
            effects: Vec<vntype_adapter::Effect>,
            comp_sink: ITfCompositionSink,
        ) -> bool {
            let client_id = *self.client_id.borrow();
            if client_id == 0 || effects.is_empty() {
                return false;
            }
            let eaten = Rc::new(RefCell::new(false));
            let session: ITfEditSession = WinEditSession::new(
                context.clone(),
                effects,
                Rc::clone(&self.tracker),
                Rc::clone(&self.composition),
                comp_sink,
                Rc::clone(&eaten),
            )
            .into();
            unsafe {
                if let Ok(hr) =
                    context.RequestEditSession(client_id, &session, TF_ES_SYNC | TF_ES_READWRITE)
                {
                    let _ = hr;
                }
            }
            let res = *eaten.borrow();
            res
        }
    }

    impl Drop for WinTextService {
        fn drop(&mut self) {
            if let Some(routing) = &self.routing {
                routing.clear_tsf_active(std::process::id());
            }
            decrement_dll_ref();
        }
    }

    impl ITfTextInputProcessor_Impl for WinTextService_Impl {
        fn Activate(&self, ptim: Ref<'_, ITfThreadMgr>, tid: u32) -> windows_core::Result<()> {
            self.ActivateEx(ptim, tid, 0)
        }

        fn Deactivate(&self) -> windows_core::Result<()> {
            if let Some(routing) = &self.routing {
                routing.clear_tsf_active(std::process::id());
            }
            if let Some(mgr) = self.thread_mgr.borrow_mut().take() {
                unsafe {
                    if let Ok(ks_mgr) = mgr.cast::<ITfKeystrokeMgr>() {
                        let _ = ks_mgr.UnadviseKeyEventSink(*self.client_id.borrow());
                    }
                    let cookie = *self.thread_mgr_cookie.borrow();
                    if cookie != 0 {
                        if let Ok(src) = mgr.cast::<ITfSource>() {
                            let _ = src.UnadviseSink(cookie);
                        }
                    }
                }
            }
            *self.thread_mgr_cookie.borrow_mut() = 0;
            *self.client_id.borrow_mut() = 0;
            *self.composition.borrow_mut() = None;
            self.tracker.borrow_mut().clear();
            Ok(())
        }
    }

    impl ITfTextInputProcessorEx_Impl for WinTextService_Impl {
        fn ActivateEx(
            &self,
            ptim: Ref<'_, ITfThreadMgr>,
            tid: u32,
            _dwflags: u32,
        ) -> windows_core::Result<()> {
            let mgr = ptim.ok()?.clone();
            *self.thread_mgr.borrow_mut() = Some(mgr.clone());
            *self.client_id.borrow_mut() = tid;

            unsafe {
                let sink: ITfThreadMgrEventSink = self.to_interface();
                if let Ok(src) = mgr.cast::<ITfSource>() {
                    let cookie = src.AdviseSink(&ITfThreadMgrEventSink::IID, &sink)?;
                    *self.thread_mgr_cookie.borrow_mut() = cookie;
                }
                let ks_mgr: ITfKeystrokeMgr = mgr.cast()?;
                let key_sink: ITfKeyEventSink = self.to_interface();
                ks_mgr.AdviseKeyEventSink(tid, &key_sink, true)?;
            }
            Ok(())
        }
    }

    impl ITfThreadMgrEventSink_Impl for WinTextService_Impl {
        fn OnInitDocumentMgr(&self, _pdim: Ref<'_, ITfDocumentMgr>) -> windows_core::Result<()> {
            Ok(())
        }

        fn OnUninitDocumentMgr(&self, _pdim: Ref<'_, ITfDocumentMgr>) -> windows_core::Result<()> {
            Ok(())
        }

        fn OnSetFocus(
            &self,
            pdimfocus: Ref<'_, ITfDocumentMgr>,
            _pdimprevfocus: Ref<'_, ITfDocumentMgr>,
        ) -> windows_core::Result<()> {
            self.maybe_reload_config();
            let (pid, app) = current_process_info();
            if pdimfocus.is_null() {
                if let Some(routing) = &self.routing {
                    routing.clear_tsf_active(pid);
                }
                return Ok(());
            }

            if let Some(routing) = &self.routing {
                routing.publish_tsf_active(pid, 0);
            }

            let mut svc = self.service.borrow_mut();
            let focus_effects = svc.controller.focus_changed(Some(&app));
            let kind = resolve_field_kind_from_scopes(&[], &app);
            let field_effects = svc.controller.set_field_kind(kind);
            drop(svc);

            if let Ok(doc_mgr) = pdimfocus.ok() {
                if let Ok(ctx) = unsafe { doc_mgr.GetTop() } {
                    let mut all = focus_effects;
                    all.extend(field_effects);
                    let _ = self.run_edit_session(&ctx, all, self.to_interface());
                }
            }
            Ok(())
        }

        fn OnPushContext(&self, _pic: Ref<'_, ITfContext>) -> windows_core::Result<()> {
            Ok(())
        }

        fn OnPopContext(&self, _pic: Ref<'_, ITfContext>) -> windows_core::Result<()> {
            Ok(())
        }
    }

    impl ITfKeyEventSink_Impl for WinTextService_Impl {
        fn OnSetFocus(&self, fforeground: BOOL) -> windows_core::Result<()> {
            if !fforeground.as_bool() {
                if let Some(routing) = &self.routing {
                    routing.clear_tsf_active(std::process::id());
                }
            }
            Ok(())
        }

        fn OnTestKeyDown(
            &self,
            _pic: Ref<'_, ITfContext>,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> windows_core::Result<BOOL> {
            self.maybe_reload_config();
            let ev = translate_tsf_key(wparam, lparam);
            let interested = !ev.ctrl
                && !ev.alt
                && !ev.meta
                && !matches!(ev.key, vntype_engine::Key::Modifier | vntype_engine::Key::Other);
            Ok(BOOL::from(interested))
        }

        fn OnTestKeyUp(
            &self,
            _pic: Ref<'_, ITfContext>,
            _wparam: WPARAM,
            _lparam: LPARAM,
        ) -> windows_core::Result<BOOL> {
            Ok(BOOL::from(false))
        }

        fn OnKeyDown(
            &self,
            pic: Ref<'_, ITfContext>,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> windows_core::Result<BOOL> {
            self.maybe_reload_config();
            let ctx = pic.ok()?;
            let ev = translate_tsf_key(wparam, lparam);
            let vk = (wparam.0 & 0xFFFF) as u32;
            let effects = self.service.borrow_mut().controller.process_key(ev, vk);
            let eaten = self.run_edit_session(ctx, effects, self.to_interface());
            Ok(BOOL::from(eaten))
        }

        fn OnKeyUp(
            &self,
            _pic: Ref<'_, ITfContext>,
            _wparam: WPARAM,
            _lparam: LPARAM,
        ) -> windows_core::Result<BOOL> {
            Ok(BOOL::from(false))
        }

        fn OnPreservedKey(
            &self,
            _pic: Ref<'_, ITfContext>,
            _rguid: *const windows_core::GUID,
        ) -> windows_core::Result<BOOL> {
            Ok(BOOL::from(false))
        }
    }

    impl ITfCompositionSink_Impl for WinTextService_Impl {
        fn OnCompositionTerminated(
            &self,
            _ecwrite: u32,
            _pcomposition: Ref<'_, ITfComposition>,
        ) -> windows_core::Result<()> {
            if !self.tracker.borrow().ending_internally {
                *self.composition.borrow_mut() = None;
                self.tracker.borrow_mut().clear();
                self.service.borrow_mut().on_composition_terminated();
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockTsfDocument {
        committed: String,
        preedit: String,
    }

    impl TsfContextBackend for MockTsfDocument {
        fn set_composition_text(&mut self, text: &str, _cursor_utf16: u32) {
            self.preedit = text.to_string();
        }
        fn end_composition_at_end(&mut self) {
            self.preedit.clear();
        }
        fn commit_text_at_selection(&mut self, text: &str) {
            self.committed.push_str(text);
            self.preedit.clear();
        }
        fn replace_before_caret(&mut self, delete_utf16: u32, insert: &str) {
            let mut u16_buf: Vec<u16> = self.committed.encode_utf16().collect();
            let keep = u16_buf.len().saturating_sub(delete_utf16 as usize);
            u16_buf.truncate(keep);
            self.committed = String::from_utf16(&u16_buf).unwrap_or_default();
            self.committed.push_str(insert);
        }
    }

    #[test]
    fn tsf_service_types_and_commits_vietnamese() {
        let mut tsf = VntypeTextService::new(Config::default());
        let mut doc = MockTsfDocument::default();
        tsf.on_set_focus("notepad", &[], &mut doc);

        for ch in "Vieejt".chars() {
            let before = doc.committed.clone();
            let eaten = tsf.on_key_down(KeyEvent::ch(ch), ch as u32, Some(&before), &mut doc);
            assert!(eaten);
        }
        assert_eq!(doc.preedit, "Việt");

        let before = doc.committed.clone();
        let eaten_space = tsf.on_key_down(KeyEvent::ch(' '), 0x20, Some(&before), &mut doc);
        assert!(eaten_space);
        assert_eq!(doc.committed, "Việt ");
        assert!(doc.preedit.is_empty());
    }
}
