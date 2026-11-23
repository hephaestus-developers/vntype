//! Kết nối D-Bus thật tới IBus Daemon bằng `zbus` cho chế độ `--ibus`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};

use serde::{Deserialize, Serialize};
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
    /// Tạo một [`IbusText`] mới từ chuỗi văn bản `text`.
    pub fn new(text: impl Into<String>) -> Self {
        let attr_list = Structure::builder()
            .add_field("IBusAttrList")
            .add_field(HashMap::<String, Variant>::new())
            .add_field(Vec::<Variant>::new())
            .build()
            .expect("tạo IBusAttrList hợp lệ");
        let attrs = OwnedVariant::try_from(Variant::from(attr_list))
            .expect("chuyển IBusAttrList sang OwnedVariant");
        IbusText { name: "IBusText".into(), attachments: HashMap::new(), text: text.into(), attrs }
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
        emit_signals(&emitter, signals).await
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
        emit_signals(&emitter, signals).await
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

    fn enable(&self) {}

    fn disable(&self) {}

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
        text: IbusText,
        cursor_pos: u32,
        visible: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn hide_preedit_text(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn commit_text(emitter: &SignalEmitter<'_>, text: IbusText) -> zbus::Result<()>;

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
}

async fn emit_signals(emitter: &SignalEmitter<'_>, signals: Vec<IbusSignal>) -> zbus::fdo::Result<()> {
    for sig in signals {
        let res = match sig {
            IbusSignal::UpdatePreeditText { text, cursor_utf16, visible } => {
                DbusEngine::update_preedit_text(emitter, IbusText::new(text), cursor_utf16, visible).await
            }
            IbusSignal::HidePreeditText => DbusEngine::hide_preedit_text(emitter).await,
            IbusSignal::CommitText { text } => DbusEngine::commit_text(emitter, IbusText::new(text)).await,
            IbusSignal::DeleteSurroundingText { offset_chars, nchars } => {
                DbusEngine::delete_surrounding_text(emitter, offset_chars, nchars).await
            }
            IbusSignal::ForwardKeyEvent { keyval, keycode, state } => {
                DbusEngine::forward_key_event(emitter, keyval, keycode, state).await
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
        let t = IbusText::new("tiếng Việt");
        let val = OwnedVariant::try_from(Variant::from(
            Structure::builder()
                .add_field(t.name)
                .add_field(t.attachments)
                .add_field(t.text)
                .add_field(t.attrs)
                .build()
                .unwrap(),
        ))
        .unwrap();
        assert_eq!(IbusText::extract_text(&val).as_deref(), Some("tiếng Việt"));
    }
}
