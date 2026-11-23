//! Bộ điều phối giao thức Wayland `zwp_input_method_v2` và `zwp_virtual_keyboard_v1`.
//!
//! Mô-đun này uỷ quyền toàn bộ quyết định Preedit / Commit / Replace / Fail-Safe cho
//! [`vntype_adapter::AdapterController`] và quản lý danh sách phím đã tiêu thụ (`consumed_keycodes`)
//! để ngăn sự kiện nhả phím (`KeyRelease`) lọt xuống ứng dụng.

use vntype_adapter::{AdapterController, Effect, utf16_len};
use vntype_engine::{Config, FieldKind, OutputMode};

use crate::xkb::WaylandKeyInput;

/// Lệnh giao thức Wayland phát ra từ bộ điều khiển để gửi qua socket Wayland.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaylandWireCommand {
    /// `zwp_input_method_v2.set_preedit_string(text, cursor_begin, cursor_end)` + `commit(serial)`.
    SetPreedit {
        /// Chuỗi gạch chân UTF-8.
        text: String,
        /// Vị trí con trỏ theo byte UTF-8.
        cursor_bytes: i32,
        /// Số thứ tự serial của phiên `done`.
        serial: u32,
    },
    /// Xoá chuỗi preedit và `commit(serial)`.
    ClearPreedit {
        /// Số thứ tự serial.
        serial: u32,
    },
    /// `commit_string(text)` + `commit(serial)`.
    Commit {
        /// Chuỗi văn bản cần chốt.
        text: String,
        /// Số thứ tự serial.
        serial: u32,
    },
    /// `delete_surrounding_text(before_length_bytes, 0)` + `commit_string(text)` + `commit(serial)`.
    Replace {
        /// Số byte UTF-8 trước con trỏ cần xoá.
        delete_bytes: u32,
        /// Chuỗi văn bản mới cần chèn.
        text: String,
        /// Số thứ tự serial.
        serial: u32,
    },
    /// Chuyển tiếp phím vật lý qua `zwp_virtual_keyboard_v1.key(time, keycode, state)`.
    ForwardVirtualKey {
        /// Mã phím evdev.
        keycode: u32,
        /// Dấu thời gian.
        time: u32,
        /// Trạng thái nhấn (`true`) hoặc nhả (`false`).
        pressed: bool,
    },
    /// Hẹn giờ chờ `ms` mili-giây trên vòng lặp sự kiện trước khi gọi `on_timer_tick`.
    ScheduleTimer {
        /// Số mili-giây cần chờ.
        ms: u32,
    },
}

/// Máy trạng thái giao thức Wayland kết nối với [`AdapterController`].
pub struct WaylandProtocolDriver {
    controller: AdapterController,
    active: bool,
    serial: u32,
    consumed_keycodes: Vec<u32>,
}

impl WaylandProtocolDriver {
    /// Khởi tạo bộ điều phối giao thức Wayland với cấu hình `config`.
    pub fn new(config: Config) -> Self {
        WaylandProtocolDriver {
            controller: AdapterController::new(config, OutputMode::Preedit),
            active: false,
            serial: 0,
            consumed_keycodes: Vec::new(),
        }
    }

    /// Bật hoặc tắt chế độ giao dịch `Replace` bất đồng bộ 2 pha có hẹn giờ.
    pub fn set_async_replace(&mut self, enabled: bool) {
        self.controller.set_async_replace(enabled);
    }

    /// Xử lý sự kiện `zwp_input_method_v2.activate`.
    pub fn on_activate(&mut self) -> Vec<WaylandWireCommand> {
        self.active = true;
        self.consumed_keycodes.clear();
        self.controller.set_surrounding_utf16(None, 0, 0);
        self.controller.reset();
        vec![WaylandWireCommand::ClearPreedit { serial: self.serial }]
    }

    /// Xử lý sự kiện `zwp_input_method_v2.deactivate`.
    pub fn on_deactivate(&mut self) -> Vec<WaylandWireCommand> {
        if !self.active {
            return Vec::new();
        }
        self.active = false;
        self.consumed_keycodes.clear();
        let effects = self.controller.flush();
        let mut cmds = self.translate_effects(effects, None);
        if !cmds.iter().any(|c| matches!(c, WaylandWireCommand::ClearPreedit { .. })) {
            cmds.push(WaylandWireCommand::ClearPreedit { serial: self.serial });
        }
        cmds
    }

    /// Xử lý sự kiện `zwp_input_method_v2.done` (tăng số thứ tự `serial`).
    pub fn on_done(&mut self) {
        self.serial = self.serial.saturating_add(1);
    }

    /// Xử lý sự kiện `zwp_input_method_v2.surrounding_text` (với `cursor_bytes` và `anchor_bytes` theo byte UTF-8).
    pub fn on_surrounding_bytes(&mut self, text: &str, cursor_bytes: u32, anchor_bytes: u32) {
        let c = cursor_bytes as usize;
        let a = anchor_bytes as usize;
        if c > text.len() || a > text.len() || !text.is_char_boundary(c) || !text.is_char_boundary(a) {
            self.controller.set_surrounding_utf16(None, 0, 0);
            return;
        }
        let cursor_u16 = utf16_len(&text[..c]);
        let anchor_u16 = utf16_len(&text[..a]);
        self.controller.set_surrounding_utf16(Some(text), cursor_u16, anchor_u16);
    }

    /// Xử lý sự kiện `zwp_input_method_v2.content_type`.
    pub fn on_content_type(&mut self, purpose: u32, hint_sensitive: bool) -> Vec<WaylandWireCommand> {
        let kind = if hint_sensitive {
            FieldKind::Password
        } else {
            match purpose {
                2..=4 | 9 => FieldKind::Number,
                5 => FieldKind::Url,
                6 => FieldKind::Email,
                8 => FieldKind::Password,
                13 => FieldKind::Terminal,
                _ => FieldKind::Text,
            }
        };
        let effects = self.controller.set_field_kind(kind);
        self.translate_effects(effects, None)
    }

    /// Xử lý một sự kiện bàn phím từ `zwp_input_method_keyboard_grab_v2`.
    pub fn on_key(&mut self, input: &WaylandKeyInput) -> Vec<WaylandWireCommand> {
        if !input.pressed {
            if let Some(pos) = self.consumed_keycodes.iter().position(|k| *k == input.keycode) {
                self.consumed_keycodes.swap_remove(pos);
                return Vec::new();
            }
            return vec![WaylandWireCommand::ForwardVirtualKey {
                keycode: input.keycode,
                time: input.time,
                pressed: false,
            }];
        }

        if !self.active {
            return vec![WaylandWireCommand::ForwardVirtualKey {
                keycode: input.keycode,
                time: input.time,
                pressed: true,
            }];
        }

        let effects = self.controller.process_key(input.to_key_event(), input.keycode);
        self.translate_effects(effects, Some(input))
    }

    /// Xử lý khi bộ hẹn giờ `ScheduleTimer` hoàn tất.
    pub fn on_timer_tick(&mut self) -> Vec<WaylandWireCommand> {
        let effects = self.controller.on_timer_tick();
        self.translate_effects(effects, None)
    }

    fn translate_effects(
        &mut self,
        effects: Vec<Effect>,
        current_input: Option<&WaylandKeyInput>,
    ) -> Vec<WaylandWireCommand> {
        let mut cmds = Vec::new();
        let mut forwarded_current = false;

        for eff in effects {
            match eff {
                Effect::SetPreedit { text, cursor_bytes, .. } => {
                    cmds.push(WaylandWireCommand::SetPreedit {
                        text,
                        cursor_bytes: cursor_bytes as i32,
                        serial: self.serial,
                    });
                }
                Effect::ClearPreedit => {
                    cmds.push(WaylandWireCommand::ClearPreedit { serial: self.serial });
                }
                Effect::CommitText { text } => {
                    cmds.push(WaylandWireCommand::Commit { text, serial: self.serial });
                }
                Effect::ReplaceText { delete_bytes, insert, .. } => {
                    cmds.push(WaylandWireCommand::Replace {
                        delete_bytes,
                        text: insert,
                        serial: self.serial,
                    });
                }
                Effect::WaitMs { ms } => {
                    cmds.push(WaylandWireCommand::ScheduleTimer { ms });
                }
                Effect::ForwardKey { raw_keycode } => {
                    if let Some(inp) = current_input {
                        if inp.keycode == raw_keycode {
                            forwarded_current = true;
                        }
                        cmds.push(WaylandWireCommand::ForwardVirtualKey {
                            keycode: raw_keycode,
                            time: inp.time,
                            pressed: true,
                        });
                    } else if raw_keycode != 0 {
                        cmds.push(WaylandWireCommand::ForwardVirtualKey {
                            keycode: raw_keycode,
                            time: 0,
                            pressed: true,
                        });
                        cmds.push(WaylandWireCommand::ForwardVirtualKey {
                            keycode: raw_keycode,
                            time: 0,
                            pressed: false,
                        });
                    }
                }
            }
        }

        if let Some(inp) = current_input
            && !forwarded_current
            && !self.consumed_keycodes.contains(&inp.keycode)
        {
            self.consumed_keycodes.push(inp.keycode);
        }
        cmds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vntype_engine::Key;

    fn key_press(ch: char, keycode: u32) -> WaylandKeyInput {
        WaylandKeyInput {
            key: Key::Char(ch),
            keycode,
            time: keycode,
            pressed: true,
            ctrl: false,
            alt: false,
            meta: false,
        }
    }

    #[test]
    fn wayland_driver_suppresses_consumed_key_releases() {
        let mut drv = WaylandProtocolDriver::new(Config::default());
        drv.on_activate();
        drv.on_done();
        let press_a = key_press('a', 30);
        let cmds = drv.on_key(&press_a);
        assert!(
            cmds.iter().any(|c| matches!(c, WaylandWireCommand::SetPreedit { text, .. } if text == "a"))
        );

        let release_a = WaylandKeyInput { pressed: false, time: 20, ..press_a };
        let rel_cmds = drv.on_key(&release_a);
        assert!(rel_cmds.is_empty(), "Phím đã bị IME tiêu thụ không được forward sự kiện nhả phím");
    }

    #[test]
    fn wayland_surrounding_deactivate_and_content_type() {
        let mut drv = WaylandProtocolDriver::new(Config::default());
        drv.on_activate();
        drv.on_done();
        drv.on_key(&key_press('v', 1));
        drv.on_key(&key_press('i', 2));
        // Deactivate phải commit chữ đang gõ dở trước khi xoá preedit
        let deact = drv.on_deactivate();
        assert!(deact.iter().any(|c| matches!(c, WaylandWireCommand::Commit { text, .. } if text == "vi")));
        assert!(deact.iter().any(|c| matches!(c, WaylandWireCommand::ClearPreedit { .. })));

        // Re-activate với surrounding bytes (UTF-8) và kiểm tra content_type sensitive
        drv.on_activate();
        drv.on_surrounding_bytes("tiêng", "tiêng".len() as u32, "tiêng".len() as u32);
        let cmds = drv.on_key(&key_press('s', 31));
        assert!(
            cmds.iter().any(|c| matches!(c, WaylandWireCommand::Replace { text, .. } if text == "ếng"))
        );

        let pw_cmds = drv.on_content_type(0, true);
        assert!(pw_cmds.iter().any(|c| matches!(c, WaylandWireCommand::ClearPreedit { .. })));
        let pass = drv.on_key(&key_press('a', 30));
        assert!(
            pass.iter().any(|c| matches!(c, WaylandWireCommand::ForwardVirtualKey { keycode: 30, .. }))
        );
    }
}
