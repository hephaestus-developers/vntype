//! Kết nối Wayland thật qua `zwp_input_method_v2` và `zwp_virtual_keyboard_v1`.

use std::io::Read;
use std::os::fd::AsFd;
use std::path::PathBuf;

use vntype_config::load_config_or_default;
use wayland_client::protocol::{wl_keyboard, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2, zwp_input_method_manager_v2, zwp_input_method_v2,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1,
};
use xkbcommon::xkb;

use crate::protocol::{WaylandProtocolDriver, WaylandWireCommand};
use crate::xkb::{WaylandKeyInput, translate_xkb_keysym};

struct WaylandApp {
    seat: Option<wl_seat::WlSeat>,
    im_manager: Option<zwp_input_method_manager_v2::ZwpInputMethodManagerV2>,
    vk_manager: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    input_method: Option<zwp_input_method_v2::ZwpInputMethodV2>,
    keyboard_grab: Option<zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2>,
    virtual_keyboard: Option<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1>,
    xkb_ctx: xkb::Context,
    xkb_state: Option<xkb::State>,
    driver: WaylandProtocolDriver,
}

impl WaylandApp {
    fn dispatch_commands(&self, cmds: Vec<WaylandWireCommand>) {
        for cmd in cmds {
            match cmd {
                WaylandWireCommand::SetPreedit { text, cursor_bytes, serial } => {
                    if let Some(im) = &self.input_method {
                        im.set_preedit_string(text, cursor_bytes, cursor_bytes);
                        im.commit(serial);
                    }
                }
                WaylandWireCommand::ClearPreedit { serial } => {
                    if let Some(im) = &self.input_method {
                        im.set_preedit_string(String::new(), 0, 0);
                        im.commit(serial);
                    }
                }
                WaylandWireCommand::Commit { text, serial } => {
                    if let Some(im) = &self.input_method {
                        im.commit_string(text);
                        im.commit(serial);
                    }
                }
                WaylandWireCommand::Replace { delete_bytes, text, serial } => {
                    if let Some(im) = &self.input_method {
                        im.delete_surrounding_text(delete_bytes, 0);
                        if !text.is_empty() {
                            im.commit_string(text);
                        }
                        im.commit(serial);
                    }
                }
                WaylandWireCommand::ForwardVirtualKey { keycode, time, pressed } => {
                    if let Some(vk) = &self.virtual_keyboard {
                        vk.key(time, keycode, u32::from(pressed));
                    }
                }
                WaylandWireCommand::ScheduleTimer { ms } => {
                    std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
                }
            }
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandApp {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_seat" => {
                    state.seat = Some(registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(7), qh, ()));
                }
                "zwp_input_method_manager_v2" => {
                    state.im_manager =
                        Some(registry.bind::<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, _, _>(
                            name,
                            1,
                            qh,
                            (),
                        ));
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    state.vk_manager = Some(
                        registry
                            .bind::<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, _, _>(
                                name,
                                1,
                                qh,
                                (),
                            ),
                    );
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandApp {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, ()> for WaylandApp {
    fn event(
        _: &mut Self,
        _: &zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
        _: zwp_input_method_manager_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, ()> for WaylandApp {
    fn event(
        _: &mut Self,
        _: &zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
        _: zwp_virtual_keyboard_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1, ()> for WaylandApp {
    fn event(
        _: &mut Self,
        _: &zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
        _: zwp_virtual_keyboard_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_input_method_v2::ZwpInputMethodV2, ()> for WaylandApp {
    fn event(
        state: &mut Self,
        _: &zwp_input_method_v2::ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_v2::Event::Activate => {
                let cmds = state.driver.on_activate();
                state.dispatch_commands(cmds);
            }
            zwp_input_method_v2::Event::Deactivate => {
                let cmds = state.driver.on_deactivate();
                state.dispatch_commands(cmds);
            }
            zwp_input_method_v2::Event::SurroundingText { text, cursor, anchor } => {
                state.driver.on_surrounding_bytes(&text, cursor, anchor);
            }
            zwp_input_method_v2::Event::ContentType { hint, purpose } => {
                let hint_bits: u32 = hint.into();
                let purpose_u32: u32 = purpose.into();
                // Bit 4 (1 << 4 = 16) là sensitive_data trong zwp_text_input_v3::content_hint
                let cmds = state.driver.on_content_type(purpose_u32, (hint_bits & 16) != 0);
                state.dispatch_commands(cmds);
            }
            zwp_input_method_v2::Event::Done => {
                state.driver.on_done();
            }
            _ => {}
        }
    }
}

impl Dispatch<zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2, ()> for WaylandApp {
    fn event(
        state: &mut Self,
        _: &zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
        event: zwp_input_method_keyboard_grab_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_keyboard_grab_v2::Event::Keymap { format, fd, size } => {
                if let Some(vk) = &state.virtual_keyboard {
                    vk.keymap(format.into(), fd.as_fd(), size);
                }
                let mut file = std::fs::File::from(fd);
                let mut buf = String::new();
                if file.read_to_string(&mut buf).is_ok() {
                    let trimmed = buf.trim_end_matches('\0');
                    if let Some(keymap) = xkb::Keymap::new_from_string(
                        &state.xkb_ctx,
                        trimmed.to_string(),
                        xkb::KEYMAP_FORMAT_TEXT_V1,
                        xkb::KEYMAP_COMPILE_NO_FLAGS,
                    ) {
                        state.xkb_state = Some(xkb::State::new(&keymap));
                    }
                }
            }
            zwp_input_method_keyboard_grab_v2::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(vk) = &state.virtual_keyboard {
                    vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
                }
                if let Some(xkb_state) = &mut state.xkb_state {
                    xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
            }
            zwp_input_method_keyboard_grab_v2::Event::Key { time, key, state: key_state, .. } => {
                let pressed =
                    matches!(key_state, wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed));
                let xkb_keycode = xkb::Keycode::new(key + 8);
                let (sym, ch, ctrl, alt, meta) = if let Some(xkb_state) = &state.xkb_state {
                    let sym = xkb_state.key_get_one_sym(xkb_keycode);
                    let utf8 = xkb_state.key_get_utf8(xkb_keycode);
                    let ch = utf8.chars().next();
                    let ctrl = xkb_state.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE);
                    let alt = xkb_state.mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE);
                    let meta = xkb_state.mod_name_is_active(xkb::MOD_NAME_LOGO, xkb::STATE_MODS_EFFECTIVE);
                    (sym.raw(), ch, ctrl, alt, meta)
                } else {
                    (0, None, false, false, false)
                };

                let input = WaylandKeyInput {
                    key: translate_xkb_keysym(sym, ch),
                    keycode: key,
                    time,
                    pressed,
                    ctrl,
                    alt,
                    meta,
                };
                let cmds = state.driver.on_key(&input);
                state.dispatch_commands(cmds);
            }
            _ => {}
        }
    }
}

/// Khởi chạy trình nền Wayland thật kết nối tới compositor qua `WAYLAND_DISPLAY`.
pub fn run_wayland_daemon(config_path: PathBuf) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("Không thể kết nối Wayland: {e}"))?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();

    let cfg = load_config_or_default(config_path);
    let mut app = WaylandApp {
        seat: None,
        im_manager: None,
        vk_manager: None,
        input_method: None,
        keyboard_grab: None,
        virtual_keyboard: None,
        xkb_ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
        xkb_state: None,
        driver: WaylandProtocolDriver::new(cfg),
    };

    let display = conn.display();
    let _registry = display.get_registry(&qh, ());
    queue.roundtrip(&mut app).map_err(|e| format!("Lỗi roundtrip Wayland: {e}"))?;

    let seat = app.seat.as_ref().ok_or("Không tìm thấy wl_seat trên compositor")?.clone();
    let im_mgr = app
        .im_manager
        .as_ref()
        .ok_or("Compositor không hỗ trợ giao thức zwp_input_method_manager_v2")?
        .clone();
    let vk_mgr = app
        .vk_manager
        .as_ref()
        .ok_or("Compositor không hỗ trợ giao thức zwp_virtual_keyboard_manager_v1")?
        .clone();

    let im = im_mgr.get_input_method(&seat, &qh, ());
    let grab = im.grab_keyboard(&qh, ());
    let vk = vk_mgr.create_virtual_keyboard(&seat, &qh, ());
    app.input_method = Some(im);
    app.keyboard_grab = Some(grab);
    app.virtual_keyboard = Some(vk);

    loop {
        queue.blocking_dispatch(&mut app).map_err(|e| format!("Lỗi vòng lặp Wayland: {e}"))?;
    }
}
