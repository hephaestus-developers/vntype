//! Kết nối D-Bus thật tới IBus Daemon bằng `zbus` cho chế độ `--ibus`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};

use serde::{Deserialize, Serialize};
use vntype_config::{Config, ToneStyle, UnicodeForm};
use zbus::object_server::SignalEmitter;
use zbus::{OwnedObjectPath, OwnedVariant, Structure, Type, Variant};

use crate::dbus_service::{IbusEngineService, IbusSignal};

/// Biểu diễn `IBusText` theo wire format D-Bus của IBus (`(sa{sv}sv)`).
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct IbusText {
    /// Tên kiểu IBusSerializable (`"IBusText"`).
    pub name: String,
    /// Bảng thuộc tính đính kèm.
    pub attachments: HashMap<String, OwnedVariant>,
    /// Nội dung chuỗi UTF-8.
    pub text: String,
    /// Danh sách thuộc tính định dạng (`IBusAttrList`).
    pub attrs: OwnedVariant,
}

impl IbusText {
    /// Tạo một [`IbusText`] mới từ chuỗi văn bản `text` (không kèm thuộc tính định dạng).
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_attrs(text, Vec::new())
    }

    /// Tạo một [`IbusText`] mới kèm gạch chân đơn (`IBUS_ATTR_TYPE_UNDERLINE`) cho toàn bộ chuỗi preedit.
    pub fn with_underline(text: impl Into<String>) -> Self {
        let s: String = text.into();
        let char_len = s.chars().count() as u32;
        let attrs = if char_len > 0 {
            let attr = Structure::builder()
                .add_field("IBusAttribute")
                .add_field(HashMap::<String, Variant>::new())
                .add_field(1u32) // IBUS_ATTR_TYPE_UNDERLINE
                .add_field(1u32) // IBUS_ATTR_UNDERLINE_SINGLE
                .add_field(0u32)
                .add_field(char_len)
                .build()
                .expect("tạo IBusAttribute hợp lệ");
            vec![Variant::from(attr)]
        } else {
            Vec::new()
        };
        Self::with_attrs(s, attrs)
    }

    fn with_attrs(text: impl Into<String>, attr_items: Vec<Variant>) -> Self {
        let attr_list = Structure::builder()
            .add_field("IBusAttrList")
            .add_field(HashMap::<String, Variant>::new())
            .add_field(attr_items)
            .build()
            .expect("tạo IBusAttrList hợp lệ");
        let attrs = OwnedVariant::try_from(Variant::from(attr_list))
            .expect("chuyển IBusAttrList sang OwnedVariant");
        IbusText { name: "IBusText".into(), attachments: HashMap::new(), text: text.into(), attrs }
    }

    /// Đóng gói `IbusText` thành `OwnedVariant` (`v`) đúng chuẩn tín hiệu D-Bus của IBus.
    pub fn into_variant(self) -> OwnedVariant {
        let s = Structure::builder()
            .add_field(self.name)
            .add_field(self.attachments)
            .add_field(self.text)
            .add_field(self.attrs)
            .build()
            .expect("tạo cấu trúc IBusText hợp lệ");
        OwnedVariant::try_from(Variant::from(s)).expect("chuyển IBusText sang OwnedVariant")
    }

    /// Trích xuất chuỗi văn bản từ một [`struct@OwnedVariant`] kiểu `IBusText` nhận qua D-Bus.
    pub fn extract_text(val: &OwnedVariant) -> Option<String> {
        let s: &Structure<'_> = val.downcast_ref().ok()?;
        let fields = s.fields();
        let text_val = fields.get(2)?;
        text_val.downcast_ref::<&str>().ok().map(str::to_string)
    }
}

enum EngineCmd {
    ProcessKey { keyval: u32, keycode: u32, state: u32, reply: mpsc::SyncSender<(bool, Vec<IbusSignal>)> },
    FocusIn { app: Option<String>, reply: mpsc::SyncSender<Vec<IbusSignal>> },
    FocusOut { reply: mpsc::SyncSender<Vec<IbusSignal>> },
    Reset { reply: mpsc::SyncSender<Vec<IbusSignal>> },
    Enable { reply: mpsc::SyncSender<Vec<IbusSignal>> },
    PropertyActivate { name: String, state: u32, reply: mpsc::SyncSender<Vec<IbusSignal>> },
    SetSurrounding { text: String, cursor: u32, anchor: u32 },
    SetContentType { purpose: u32, hints: u32, reply: mpsc::SyncSender<Vec<IbusSignal>> },
}

fn spawn_engine_actor(config_path: PathBuf) -> Mutex<mpsc::Sender<EngineCmd>> {
    let (tx, rx) = mpsc::channel::<EngineCmd>();
    std::thread::spawn(move || {
        let mut svc = IbusEngineService::new(config_path);
        while let Ok(cmd) = rx.recv() {
            match cmd {
                EngineCmd::ProcessKey { keyval, keycode, state, reply } => {
                    let handled = svc.process_key_event(keyval, keycode, state);
                    let _ = reply.send((handled, svc.drain_signals()));
                }
                EngineCmd::FocusIn { app, reply } => {
                    svc.focus_in(app.as_deref());
                    let _ = reply.send(svc.drain_signals());
                }
                EngineCmd::FocusOut { reply } => {
                    svc.focus_out();
                    let _ = reply.send(svc.drain_signals());
                }
                EngineCmd::Reset { reply } => {
                    svc.reset();
                    let _ = reply.send(svc.drain_signals());
                }
                EngineCmd::Enable { reply } => {
                    svc.emit_register_menu();
                    let _ = reply.send(svc.drain_signals());
                }
                EngineCmd::PropertyActivate { name, state, reply } => {
                    svc.property_activate(&name, state);
                    let _ = reply.send(svc.drain_signals());
                }
                EngineCmd::SetSurrounding { text, cursor, anchor } => {
                    svc.set_surrounding_text(&text, cursor, anchor);
                }
                EngineCmd::SetContentType { purpose, hints, reply } => {
                    svc.set_content_type(purpose, hints);
                    let _ = reply.send(svc.drain_signals());
                }
            }
        }
    });
    Mutex::new(tx)
}

struct DbusEngine {
    tx: Mutex<mpsc::Sender<EngineCmd>>,
}

impl DbusEngine {
    fn send_cmd(&self, cmd: EngineCmd) -> zbus::fdo::Result<()> {
        let tx = self.tx.lock().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        tx.send(cmd).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
}

#[zbus::interface(name = "org.freedesktop.IBus.Engine")]
impl DbusEngine {
    async fn process_key_event(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::fdo::Result<bool> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::ProcessKey { keyval, keycode, state, reply: reply_tx })?;
        let (handled, signals) = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await?;
        Ok(handled)
    }

    async fn focus_in(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::FocusIn { app: None, reply: reply_tx })?;
        let signals = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await?;
        let _ = DbusEngine::require_surrounding_text(&emitter).await;
        Ok(())
    }

    async fn focus_in_id(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        _object_path: &str,
        client: &str,
    ) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::FocusIn { app: Some(client.to_string()), reply: reply_tx })?;
        let signals = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await?;
        let _ = DbusEngine::require_surrounding_text(&emitter).await;
        Ok(())
    }

    async fn focus_out(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::FocusOut { reply: reply_tx })?;
        let signals = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await
    }

    async fn focus_out_id(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        _object_path: &str,
    ) -> zbus::fdo::Result<()> {
        self.focus_out(emitter).await
    }

    async fn reset(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::Reset { reply: reply_tx })?;
        let signals = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await
    }

    async fn enable(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::Enable { reply: reply_tx })?;
        let signals = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await?;
        let _ = DbusEngine::require_surrounding_text(&emitter).await;
        Ok(())
    }

    fn disable(&self) {}

    fn set_capabilities(&self, _caps: u32) {}

    fn set_cursor_location(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}

    fn set_cursor_location_relative(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}

    async fn property_activate(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        name: &str,
        state: u32,
    ) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::PropertyActivate { name: name.to_string(), state, reply: reply_tx })?;
        let signals = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await
    }

    fn set_surrounding_text(&self, text: OwnedVariant, cursor_pos: u32, anchor_pos: u32) {
        if let Some(s) = IbusText::extract_text(&text) {
            let _ = self.send_cmd(EngineCmd::SetSurrounding {
                text: s,
                cursor: cursor_pos,
                anchor: anchor_pos,
            });
        }
    }

    async fn set_content_type(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        purpose: u32,
        hints: u32,
    ) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_cmd(EngineCmd::SetContentType { purpose, hints, reply: reply_tx })?;
        let signals = reply_rx.recv().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        emit_signals(&emitter, signals).await
    }

    fn destroy(&self) {}

    #[zbus(signal)]
    async fn update_preedit_text(
        emitter: &SignalEmitter<'_>,
        text: OwnedVariant,
        cursor_pos: u32,
        visible: bool,
        mode: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn hide_preedit_text(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn commit_text(emitter: &SignalEmitter<'_>, text: OwnedVariant) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn delete_surrounding_text(
        emitter: &SignalEmitter<'_>,
        offset_from_cursor: i32,
        nchars: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn forward_key_event(
        emitter: &SignalEmitter<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn require_surrounding_text(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn register_properties(emitter: &SignalEmitter<'_>, props: OwnedVariant) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_property(emitter: &SignalEmitter<'_>, prop: OwnedVariant) -> zbus::Result<()>;
}

fn prop_list_variant(items: Vec<Structure<'static>>) -> OwnedVariant {
    let vars: Vec<Variant> = items.into_iter().map(Variant::from).collect();
    let list = Structure::builder()
        .add_field("IBusPropList")
        .add_field(HashMap::<String, Variant>::new())
        .add_field(vars)
        .build()
        .expect("tạo IBusPropList hợp lệ");
    OwnedVariant::try_from(Variant::from(list)).expect("chuyển IBusPropList sang OwnedVariant")
}

fn build_ibus_property_with_sub(
    key: &str,
    prop_type: u32,
    label: &str,
    tooltip: &str,
    state: u32,
    sub_props: Vec<Structure<'static>>,
) -> Structure<'static> {
    Structure::builder()
        .add_field("IBusProperty")
        .add_field(HashMap::<String, Variant>::new())
        .add_field(key.to_string())
        .add_field(prop_type)
        .add_field(IbusText::new(label).into_variant())
        .add_field(String::new())
        .add_field(IbusText::new(tooltip).into_variant())
        .add_field(true)
        .add_field(true)
        .add_field(state)
        .add_field(prop_list_variant(sub_props))
        .add_field(IbusText::new("").into_variant())
        .build()
        .expect("tạo IBusProperty hợp lệ")
}

fn prop_radio(key: &str, label: &str, checked: bool) -> Structure<'static> {
    build_ibus_property_with_sub(key, 2, label, label, u32::from(checked), Vec::new())
}

fn prop_toggle(key: &str, label: &str, checked: bool) -> Structure<'static> {
    build_ibus_property_with_sub(key, 1, label, label, u32::from(checked), Vec::new())
}

fn prop_separator(key: &str) -> Structure<'static> {
    build_ibus_property_with_sub(key, 4, "", "", 0, Vec::new())
}

fn prop_submenu(key: &str, label: &str, children: Vec<Structure<'static>>) -> Structure<'static> {
    build_ibus_property_with_sub(key, 3, label, label, 0, children)
}

fn build_changed_properties(changed_key: &str, cfg: &Config) -> Vec<Structure<'static>> {
    let method = cfg.input_method.trim().to_ascii_lowercase();
    if changed_key.starts_with("vntype.im.") {
        return vec![
            prop_radio("vntype.im.telex", "Telex", method == "telex"),
            prop_radio("vntype.im.vni", "VNI", method == "vni"),
            prop_radio("vntype.im.simple_telex", "Simple Telex", method == "simple_telex"),
            prop_radio("vntype.im.viqr", "VIQR", method == "viqr"),
        ];
    }
    if changed_key.starts_with("vntype.charset.") {
        return vec![
            prop_radio(
                "vntype.charset.nfc",
                "Unicode dựng sẵn (NFC)",
                cfg.unicode_form == UnicodeForm::Nfc,
            ),
            prop_radio("vntype.charset.nfd", "Unicode tổ hợp (NFD)", cfg.unicode_form == UnicodeForm::Nfd),
        ];
    }
    if changed_key.starts_with("vntype.tone.") {
        return vec![
            prop_radio("vntype.tone.new", "Đặt dấu mới (oà, uý)", cfg.tone_style == ToneStyle::New),
            prop_radio("vntype.tone.old", "Đặt dấu cũ (òa, úy)", cfg.tone_style == ToneStyle::Old),
        ];
    }
    let single = match changed_key {
        "vntype.opt.spell_check" => {
            Some(prop_toggle("vntype.opt.spell_check", "Kiểm tra chính tả", cfg.spell_check))
        }
        "vntype.opt.w_as_u" => Some(prop_toggle("vntype.opt.w_as_u", "Cho phép w → ư", cfg.w_as_u)),
        "vntype.opt.bracket_keys" => {
            Some(prop_toggle("vntype.opt.bracket_keys", "Phím [ ] → ơ, ư", cfg.bracket_keys))
        }
        "vntype.opt.quick_telex" => {
            Some(prop_toggle("vntype.opt.quick_telex", "Gõ tắt phụ âm đầu (Quick Telex)", cfg.quick_telex))
        }
        "vntype.opt.allow_coda_k" => {
            Some(prop_toggle("vntype.opt.allow_coda_k", "Cho phép phụ âm cuối k", cfg.allow_coda_k))
        }
        "vntype.opt.edit_previous_word" => Some(prop_toggle(
            "vntype.opt.edit_previous_word",
            "Sửa từ trước con trỏ",
            cfg.edit_previous_word,
        )),
        "vntype.opt.restore_after_space" => Some(prop_toggle(
            "vntype.opt.restore_after_space",
            "Khôi phục từ khi xoá Space",
            cfg.restore_after_space,
        )),
        "vntype.opt.auto_capitalize" => {
            Some(prop_toggle("vntype.opt.auto_capitalize", "Tự viết hoa đầu câu", cfg.auto_capitalize))
        }
        "vntype.opt.smart_caps" => {
            Some(prop_toggle("vntype.opt.smart_caps", "Giữ chữ hoa thông minh", cfg.smart_caps))
        }
        "vntype.macro.enabled" => {
            Some(prop_toggle("vntype.macro.enabled", "Bật gõ tắt (Macro)", cfg.macro_enabled))
        }
        "vntype.macro.undo" => {
            Some(prop_toggle("vntype.macro.undo", "Backspace hoàn tác gõ tắt", cfg.macro_undo))
        }
        "vntype.macro.digits" => {
            Some(prop_toggle("vntype.macro.digits", "Cho phép chữ số trong gõ tắt", cfg.macro_digits))
        }
        "vntype.macro.in_off_mode" => Some(prop_toggle(
            "vntype.macro.in_off_mode",
            "Bung gõ tắt khi tắt tiếng Việt",
            cfg.macro_in_off_mode,
        )),
        _ => None,
    };
    single.into_iter().collect()
}

fn build_ibus_prop_list_variant(cfg: &Config) -> OwnedVariant {
    let method = cfg.input_method.trim().to_ascii_lowercase();

    let top_items = vec![
        prop_submenu(
            "vntype.menu.input_method",
            "Kiểu gõ",
            vec![
                prop_radio("vntype.im.telex", "Telex", method == "telex"),
                prop_radio("vntype.im.vni", "VNI", method == "vni"),
                prop_radio("vntype.im.simple_telex", "Simple Telex", method == "simple_telex"),
                prop_radio("vntype.im.viqr", "VIQR", method == "viqr"),
            ],
        ),
        prop_submenu(
            "vntype.menu.unicode_form",
            "Bảng mã",
            vec![
                prop_radio(
                    "vntype.charset.nfc",
                    "Unicode dựng sẵn (NFC)",
                    cfg.unicode_form == UnicodeForm::Nfc,
                ),
                prop_radio(
                    "vntype.charset.nfd",
                    "Unicode tổ hợp (NFD)",
                    cfg.unicode_form == UnicodeForm::Nfd,
                ),
            ],
        ),
        prop_submenu(
            "vntype.menu.tone_style",
            "Kiểu bỏ dấu",
            vec![
                prop_radio("vntype.tone.new", "Đặt dấu mới (oà, uý)", cfg.tone_style == ToneStyle::New),
                prop_radio("vntype.tone.old", "Đặt dấu cũ (òa, úy)", cfg.tone_style == ToneStyle::Old),
            ],
        ),
        prop_separator("vntype.sep1"),
        prop_submenu(
            "vntype.menu.options",
            "Tuỳ chọn bộ gõ",
            vec![
                prop_toggle("vntype.opt.spell_check", "Kiểm tra chính tả", cfg.spell_check),
                prop_toggle("vntype.opt.w_as_u", "Cho phép w → ư", cfg.w_as_u),
                prop_toggle("vntype.opt.bracket_keys", "Phím [ ] → ơ, ư", cfg.bracket_keys),
                prop_toggle("vntype.opt.quick_telex", "Gõ tắt phụ âm đầu (Quick Telex)", cfg.quick_telex),
                prop_toggle("vntype.opt.allow_coda_k", "Cho phép phụ âm cuối k", cfg.allow_coda_k),
                prop_separator("vntype.opt.sep1"),
                prop_toggle(
                    "vntype.opt.edit_previous_word",
                    "Sửa từ trước con trỏ",
                    cfg.edit_previous_word,
                ),
                prop_toggle(
                    "vntype.opt.restore_after_space",
                    "Khôi phục từ khi xoá Space",
                    cfg.restore_after_space,
                ),
                prop_separator("vntype.opt.sep2"),
                prop_toggle("vntype.opt.auto_capitalize", "Tự viết hoa đầu câu", cfg.auto_capitalize),
                prop_toggle("vntype.opt.smart_caps", "Giữ chữ hoa thông minh", cfg.smart_caps),
            ],
        ),
        prop_submenu(
            "vntype.menu.macros",
            "Gõ tắt (Macro)",
            vec![
                prop_toggle("vntype.macro.enabled", "Bật gõ tắt (Macro)", cfg.macro_enabled),
                prop_toggle("vntype.macro.undo", "Backspace hoàn tác gõ tắt", cfg.macro_undo),
                prop_toggle("vntype.macro.digits", "Cho phép chữ số trong gõ tắt", cfg.macro_digits),
                prop_toggle(
                    "vntype.macro.in_off_mode",
                    "Bung gõ tắt khi tắt tiếng Việt",
                    cfg.macro_in_off_mode,
                ),
            ],
        ),
    ];

    prop_list_variant(top_items)
}

fn utf16_offset_to_chars(text: &str, utf16_off: u32) -> u32 {
    let mut u16_acc = 0u32;
    let mut chars = 0u32;
    for ch in text.chars() {
        if u16_acc >= utf16_off {
            break;
        }
        u16_acc += ch.len_utf16() as u32;
        chars += 1;
    }
    chars
}

async fn emit_signals(emitter: &SignalEmitter<'_>, signals: Vec<IbusSignal>) -> zbus::fdo::Result<()> {
    for sig in signals {
        let res = match sig {
            IbusSignal::UpdatePreeditText { text, cursor_utf16, visible } => {
                let cursor_chars = utf16_offset_to_chars(&text, cursor_utf16);
                DbusEngine::update_preedit_text(
                    emitter,
                    IbusText::with_underline(text).into_variant(),
                    cursor_chars,
                    visible,
                    0, // IBUS_ENGINE_PREEDIT_CLEAR
                )
                .await
            }
            IbusSignal::HidePreeditText => DbusEngine::hide_preedit_text(emitter).await,
            IbusSignal::CommitText { text } => {
                DbusEngine::commit_text(emitter, IbusText::new(text).into_variant()).await
            }
            IbusSignal::DeleteSurroundingText { offset_chars, nchars } => {
                DbusEngine::delete_surrounding_text(emitter, offset_chars, nchars).await
            }
            IbusSignal::ForwardKeyEvent { keyval, keycode, state } => {
                DbusEngine::forward_key_event(emitter, keyval, keycode, state).await
            }
            IbusSignal::RegisterMenu { config } => {
                let props = build_ibus_prop_list_variant(&config);
                DbusEngine::register_properties(emitter, props).await
            }
            IbusSignal::UpdateMenu { changed_key, config } => {
                for prop in build_changed_properties(&changed_key, &config) {
                    let v = OwnedVariant::try_from(Variant::from(prop))
                        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
                    if let Err(err) = DbusEngine::update_property(emitter, v).await {
                        return Err(zbus::fdo::Error::Failed(err.to_string()));
                    }
                }
                Ok(())
            }
        };
        if let Err(err) = res {
            return Err(zbus::fdo::Error::Failed(err.to_string()));
        }
    }
    Ok(())
}

struct DbusFactory {
    config_path: PathBuf,
    next_id: AtomicU64,
}

#[zbus::interface(name = "org.freedesktop.IBus.Factory")]
impl DbusFactory {
    async fn create_engine(
        &self,
        #[zbus(connection)] conn: &zbus::Connection,
        engine_name: &str,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        if engine_name != "vntype" {
            return Err(zbus::fdo::Error::InvalidArgs(format!("Engine không hỗ trợ: {engine_name}")));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let path_str = format!("/org/freedesktop/IBus/Engine/{id}");
        let path = OwnedObjectPath::try_from(path_str.as_str())
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let engine = DbusEngine { tx: spawn_engine_actor(self.config_path.clone()) };
        conn.object_server()
            .at(&path, engine)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(path)
    }
}

/// Tìm địa chỉ D-Bus socket của IBus Daemon hiện hành.
pub fn resolve_ibus_address() -> Result<String, String> {
    if let Ok(addr) = std::env::var("IBUS_ADDRESS")
        && !addr.trim().is_empty()
    {
        return Ok(addr);
    }
    let out = Command::new("ibus")
        .arg("address")
        .output()
        .map_err(|e| format!("Không thể chạy lệnh `ibus address`: {e}"))?;
    let addr = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() && !addr.is_empty() {
        Ok(addr)
    } else {
        Err("IBus Daemon chưa chạy hoặc không tìm thấy địa chỉ IBus D-Bus".into())
    }
}

struct ThreadWaker(std::thread::Thread);

impl std::task::Wake for ThreadWaker {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let waker = std::task::Waker::from(std::sync::Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = std::task::Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(val) => return val,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

/// Khởi chạy trình nền `org.freedesktop.IBus.VNType` kết nối D-Bus thật tới IBus Daemon.
pub fn run_ibus_daemon(config_path: PathBuf) -> Result<(), String> {
    let addr = resolve_ibus_address()?;
    let factory = DbusFactory { config_path, next_id: AtomicU64::new(1) };
    let _conn = block_on(async {
        zbus::connection::Builder::address(addr.as_str())
            .name("org.freedesktop.IBus.VNType")
            .serve_at("/org/freedesktop/IBus/Factory", factory)
            .build()
            .await
            .map_err(|e| format!("Không thể kết nối IBus D-Bus: {e}"))
    })?;

    loop {
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ibus_text_roundtrip_wire_value() {
        let val = IbusText::new("tiếng Việt").into_variant();
        assert_eq!(IbusText::extract_text(&val).as_deref(), Some("tiếng Việt"));

        let underlined = IbusText::with_underline("tiếng").into_variant();
        assert_eq!(IbusText::extract_text(&underlined).as_deref(), Some("tiếng"));
        assert_eq!(utf16_offset_to_chars("😀a", 3), 2);

        let prop_list = build_ibus_prop_list_variant(&Config::default());
        let s: &Structure<'_> = prop_list.downcast_ref().unwrap();
        assert_eq!(s.fields()[0].downcast_ref::<&str>().ok(), Some("IBusPropList"));
        assert_eq!(build_changed_properties("vntype.opt.w_as_u", &Config::default()).len(), 1);
    }
}
