//! Giao diện đồ họa hiện đại bằng `eframe`/`egui` cho ứng dụng cấu hình VNType.

#[cfg(windows)]
pub mod win_gui {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use eframe::egui;
    use vntype_config::{Config, SwitchKey, ToneStyle, UnicodeForm};
    use vntype_win_ipc::win_shm::SharedRoutingMapping;

    use crate::SettingsViewModel;

    const WINDOW_W: f32 = 540.0;
    const CONTENT_W: f32 = 512.0;
    const COMPACT_H: f32 = 168.0;
    const EXPANDED_H: f32 = 396.0;

    /// Cầu nối đồng bộ giữa luồng nền (`vntype-hook`) và cửa sổ `eframe` trên luồng chính.
    pub struct HookGuiBridge {
        show_requested: AtomicBool,
        exit_requested: AtomicBool,
        refresh_requested: AtomicBool,
        vietnamese_enabled: AtomicBool,
        egui_ctx: Mutex<Option<egui::Context>>,
    }

    impl Default for HookGuiBridge {
        fn default() -> Self {
            Self {
                show_requested: AtomicBool::new(false),
                exit_requested: AtomicBool::new(false),
                refresh_requested: AtomicBool::new(false),
                vietnamese_enabled: AtomicBool::new(true),
                egui_ctx: Mutex::new(None),
            }
        }
    }

    impl HookGuiBridge {
        /// Tạo mới cầu nối dùng chung giữa luồng Hook/Tray và luồng giao diện.
        pub fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }

        fn wake(&self) {
            if let Ok(guard) = self.egui_ctx.lock() {
                if let Some(ctx) = guard.as_ref() {
                    ctx.request_repaint();
                }
            }
        }

        /// Yêu cầu hiển thị và đưa cửa sổ cấu hình lên phía trước.
        pub fn request_show(&self) {
            self.show_requested.store(true, Ordering::Release);
            unsafe {
                let mut title: Vec<u16> =
                    concat!("VNType ", env!("CARGO_PKG_VERSION")).encode_utf16().collect();
                title.push(0);
                let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
                    core::ptr::null(),
                    title.as_ptr(),
                );
                if !hwnd.is_null() {
                    windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                        hwnd,
                        windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
                    );
                    windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
                }
            }
            self.wake();
        }

        /// Yêu cầu thoát hoàn toàn ứng dụng GUI.
        pub fn request_exit(&self) {
            self.exit_requested.store(true, Ordering::Release);
            self.wake();
        }

        /// Báo cho GUI nạp lại cấu hình từ đĩa (khi người dùng đổi kiểu gõ từ menu khay).
        pub fn request_refresh(&self) {
            self.refresh_requested.store(true, Ordering::Release);
            self.wake();
        }

        /// Cập nhật trạng thái bật/tắt Tiếng Việt từ luồng Hook/Tray để hiển thị lên GUI.
        pub fn set_vietnamese_enabled(&self, enabled: bool) {
            self.vietnamese_enabled.store(enabled, Ordering::Release);
            self.wake();
        }

        /// Đọc trạng thái bật/tắt Tiếng Việt hiện tại.
        pub fn is_vietnamese_enabled(&self) -> bool {
            self.vietnamese_enabled.load(Ordering::Acquire)
        }
    }

    struct VntypeSettingsApp {
        vm: SettingsViewModel,
        routing: Option<SharedRoutingMapping>,
        expanded: bool,
        vietnamese_ui: bool,
        bridge: Option<Arc<HookGuiBridge>>,
        on_toggle: Option<Box<dyn Fn() + Send>>,
        on_config_changed: Option<Box<dyn Fn() + Send>>,
        on_quit: Option<Box<dyn Fn() + Send>>,
        allow_close: bool,
    }

    impl VntypeSettingsApp {
        fn new(
            cc: &eframe::CreationContext<'_>,
            vm: SettingsViewModel,
            bridge: Option<Arc<HookGuiBridge>>,
            on_toggle: Option<Box<dyn Fn() + Send>>,
            on_config_changed: Option<Box<dyn Fn() + Send>>,
            on_quit: Option<Box<dyn Fn() + Send>>,
        ) -> Self {
            setup_vietnamese_fonts(&cc.egui_ctx);
            apply_light_windows_theme(&cc.egui_ctx);
            if let Some(b) = &bridge {
                if let Ok(mut guard) = b.egui_ctx.lock() {
                    *guard = Some(cc.egui_ctx.clone());
                }
            }
            let routing = SharedRoutingMapping::open_or_create().ok();
            Self {
                vm,
                routing,
                expanded: false,
                vietnamese_ui: true,
                bridge,
                on_toggle,
                on_config_changed,
                on_quit,
                allow_close: false,
            }
        }

        fn save_and_notify(&mut self) {
            if self.vm.apply_and_save().is_ok() {
                if let Some(routing) = &self.routing {
                    routing.notify_config_reloaded();
                }
                if let Some(cb) = &self.on_config_changed {
                    cb();
                }
            }
        }

        fn target_height(&self) -> f32 {
            if self.expanded { EXPANDED_H } else { COMPACT_H }
        }

        fn hide_or_close(&mut self, ctx: &egui::Context) {
            if self.bridge.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        fn quit_app(&mut self, ctx: &egui::Context) {
            self.allow_close = true;
            if let Some(on_quit) = &self.on_quit {
                on_quit();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn setup_vietnamese_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
            fonts
                .font_data
                .insert("segoe_ui".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "segoe_ui".to_owned());
        }
        ctx.set_fonts(fonts);
    }

    fn apply_light_windows_theme(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = egui::Color32::from_rgb(243, 243, 243);
        visuals.window_fill = egui::Color32::from_rgb(243, 243, 243);
        visuals.extreme_bg_color = egui::Color32::WHITE;
        visuals.faint_bg_color = egui::Color32::from_rgb(249, 249, 249);
        visuals.selection.bg_fill = egui::Color32::from_rgb(0, 103, 192);
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);

        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(249, 249, 249);
        visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(218, 218, 218));
        visuals.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(32, 32, 32));

        visuals.widgets.inactive.bg_fill = egui::Color32::WHITE;
        visuals.widgets.inactive.weak_bg_fill = egui::Color32::WHITE;
        visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(204, 204, 204));
        visuals.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(32, 32, 32));

        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(235, 243, 251);
        visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(235, 243, 251);
        visuals.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0, 103, 192));
        visuals.widgets.hovered.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(16, 16, 16));

        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(220, 234, 248);
        visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(220, 234, 248);
        visuals.widgets.active.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0, 90, 170));

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(13.5, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(13.5, egui::FontFamily::Proportional),
        );
        ctx.set_style(style);
    }

    #[derive(Clone, Copy)]
    enum ButtonIcon {
        Check,
        Exit,
        Expand,
        Collapse,
        Reset,
    }

    fn draw_vector_icon(painter: &egui::Painter, rect: egui::Rect, icon: ButtonIcon) {
        let c = rect.center();
        match icon {
            ButtonIcon::Check => {
                let color = egui::Color32::from_rgb(104, 179, 32);
                let stroke = egui::Stroke::new(2.4_f32, color);
                painter.line_segment(
                    [egui::pos2(c.x - 5.5, c.y + 0.5), egui::pos2(c.x - 1.5, c.y + 4.5)],
                    stroke,
                );
                painter.line_segment(
                    [egui::pos2(c.x - 1.5, c.y + 4.5), egui::pos2(c.x + 6.0, c.y - 4.0)],
                    stroke,
                );
            }
            ButtonIcon::Exit => {
                let red = egui::Color32::from_rgb(211, 54, 48);
                let door = egui::Rect::from_min_max(
                    egui::pos2(c.x - 6.0, c.y - 6.0),
                    egui::pos2(c.x + 1.5, c.y + 6.0),
                );
                painter.rect_stroke(door, 1.5, egui::Stroke::new(1.6_f32, red), egui::StrokeKind::Inside);
                let arrow = egui::Stroke::new(1.8_f32, red);
                painter.line_segment([egui::pos2(c.x - 1.0, c.y), egui::pos2(c.x + 6.5, c.y)], arrow);
                painter.line_segment(
                    [egui::pos2(c.x + 3.5, c.y - 3.0), egui::pos2(c.x + 6.5, c.y)],
                    arrow,
                );
                painter.line_segment(
                    [egui::pos2(c.x + 3.5, c.y + 3.0), egui::pos2(c.x + 6.5, c.y)],
                    arrow,
                );
            }
            ButtonIcon::Expand | ButtonIcon::Collapse => {
                let blue = egui::Color32::from_rgb(24, 144, 232);
                let stroke = egui::Stroke::new(2.0_f32, blue);
                painter.line_segment([egui::pos2(c.x, c.y - 5.5), egui::pos2(c.x, c.y + 5.5)], stroke);
                if matches!(icon, ButtonIcon::Expand) {
                    painter.line_segment(
                        [egui::pos2(c.x - 4.0, c.y + 1.5), egui::pos2(c.x, c.y + 5.5)],
                        stroke,
                    );
                    painter.line_segment(
                        [egui::pos2(c.x + 4.0, c.y + 1.5), egui::pos2(c.x, c.y + 5.5)],
                        stroke,
                    );
                } else {
                    painter.line_segment(
                        [egui::pos2(c.x - 4.0, c.y - 1.5), egui::pos2(c.x, c.y - 5.5)],
                        stroke,
                    );
                    painter.line_segment(
                        [egui::pos2(c.x + 4.0, c.y - 1.5), egui::pos2(c.x, c.y - 5.5)],
                        stroke,
                    );
                }
            }
            ButtonIcon::Reset => {
                let gray = egui::Color32::from_rgb(80, 80, 80);
                let stroke = egui::Stroke::new(1.8_f32, gray);
                let r = 5.5_f32;
                let pts: Vec<egui::Pos2> = (0..=14)
                    .map(|i| {
                        let a = -0.3 + (i as f32 / 14.0) * 4.6;
                        egui::pos2(c.x + r * a.cos(), c.y + r * a.sin())
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, stroke));
                painter.line_segment(
                    [egui::pos2(c.x + 2.5, c.y - 5.5), egui::pos2(c.x + 6.0, c.y - 2.0)],
                    stroke,
                );
            }
        }
    }

    fn icon_button(
        ui: &mut egui::Ui,
        size: egui::Vec2,
        icon: ButtonIcon,
        label: &str,
        accent_border: bool,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let stroke = if accent_border && !response.hovered() {
                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 103, 192))
            } else {
                visuals.bg_stroke
            };
            let painter = ui.painter();
            painter.rect(
                rect,
                4.0,
                visuals.bg_fill,
                stroke,
                egui::StrokeKind::Inside,
            );
            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 18.0, rect.center().y),
                egui::vec2(16.0, 16.0),
            );
            draw_vector_icon(painter, icon_rect, icon);
            let galley = painter.layout_no_wrap(
                label.to_owned(),
                egui::FontId::new(13.5, egui::FontFamily::Proportional),
                visuals.text_color(),
            );
            let text_pos = egui::pos2(
                rect.left() + 34.0,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley, visuals.text_color());
        }
        response
    }

    fn toggle_switch(ui: &mut egui::Ui, on: bool) -> egui::Response {
        let desired_size = egui::vec2(38.0, 20.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let how_on = ui.ctx().animate_bool_responsive(response.id, on);
            let painter = ui.painter();
            let radius = 0.5 * rect.height();
            let (track_fill, track_stroke, thumb_color) = if on {
                let fill = if response.hovered() {
                    egui::Color32::from_rgb(0, 90, 170)
                } else {
                    egui::Color32::from_rgb(0, 103, 192)
                };
                (fill, egui::Stroke::NONE, egui::Color32::WHITE)
            } else {
                let fill = if response.hovered() {
                    egui::Color32::from_rgb(232, 232, 232)
                } else {
                    egui::Color32::from_rgb(243, 243, 243)
                };
                (
                    fill,
                    egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(118, 118, 118)),
                    egui::Color32::from_rgb(92, 92, 92),
                )
            };
            painter.rect(rect, radius, track_fill, track_stroke, egui::StrokeKind::Inside);
            let thumb_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
            let thumb_r = if response.is_pointer_button_down_on() { 6.5 } else { 6.0 };
            painter.circle_filled(egui::pos2(thumb_x, rect.center().y), thumb_r, thumb_color);
        }
        response
    }

    fn section_card_with_header(
        ui: &mut egui::Ui,
        title: &str,
        outer_width: f32,
        header_right: impl FnOnce(&mut egui::Ui),
        add_contents: impl FnOnce(&mut egui::Ui),
    ) -> egui::Response {
        let frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(249, 249, 249))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(218, 218, 218)))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 10));
        frame
            .show(ui, |ui| {
                ui.set_width((outer_width - 24.0).max(0.0));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(title)
                                .strong()
                                .color(egui::Color32::from_rgb(45, 45, 45)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), header_right);
                    });
                    ui.add_space(4.0);
                    add_contents(ui);
                });
            })
            .response
    }

    fn section_card(ui: &mut egui::Ui, title: &str, outer_width: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
        let _ = section_card_with_header(ui, title, outer_width, |_| {}, add_contents);
    }

    fn build_app_icon() -> egui::IconData {
        // Biểu tượng chữ "V" đơn giản trên nền đỏ bo góc 32x32 RGBA.
        const S: usize = 32;
        let mut rgba = vec![0u8; S * S * 4];
        for y in 0..S {
            for x in 0..S {
                let dx = (x as f32 + 0.5) - (S as f32 / 2.0);
                let dy = (y as f32 + 0.5) - (S as f32 / 2.0);
                let in_badge = dx.abs() <= 14.0 && dy.abs() <= 14.0;
                let idx = (y * S + x) * 4;
                if in_badge {
                    // Vẽ chữ V màu trắng ở giữa.
                    let ny = y as f32;
                    let nx = x as f32;
                    let left_arm = (ny - 7.0) * 0.42 + 8.5;
                    let right_arm = 22.5 - (ny - 7.0) * 0.42;
                    let is_v = (7.0..=24.0).contains(&ny)
                        && ((nx - left_arm).abs() <= 1.8 || (nx - right_arm).abs() <= 1.8);
                    if is_v {
                        rgba[idx] = 255;
                        rgba[idx + 1] = 255;
                        rgba[idx + 2] = 255;
                        rgba[idx + 3] = 255;
                    } else {
                        rgba[idx] = 211;
                        rgba[idx + 1] = 47;
                        rgba[idx + 2] = 47;
                        rgba[idx + 3] = 255;
                    }
                }
            }
        }
        egui::IconData { rgba, width: S as u32, height: S as u32 }
    }

    impl eframe::App for VntypeSettingsApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            if let Some(bridge) = self.bridge.clone() {
                if bridge.exit_requested.swap(false, Ordering::AcqRel) {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                if bridge.refresh_requested.swap(false, Ordering::AcqRel) {
                    self.vm = SettingsViewModel::load(self.vm.config_path.clone());
                }
                if bridge.show_requested.swap(false, Ordering::AcqRel) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }

            if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
                if self.bridge.is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    return;
                }
            }

            let mut changed = false;
            let vi = self.vietnamese_ui;

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::central_panel(&ctx.style())
                        .fill(egui::Color32::from_rgb(243, 243, 243))
                        .inner_margin(egui::Margin::same(14)),
                )
                .show(ctx, |ui| {
                    ui.set_width(CONTENT_W);
                    ui.horizontal_top(|ui| {
                        let ctrl_title = if vi { "Điều khiển" } else { "Settings" };
                        let vn_enabled = self
                            .bridge
                            .as_ref()
                            .map(|b| b.is_vietnamese_enabled())
                            .unwrap_or(self.vm.draft.default_vietnamese);
                        let on_toggle = &self.on_toggle;
                        let ctrl_resp = section_card_with_header(
                            ui,
                            ctrl_title,
                            392.0,
                            |ui| {
                                if toggle_switch(ui, vn_enabled).clicked() {
                                    if let Some(cb) = on_toggle {
                                        cb();
                                    }
                                }
                                let status_text = match (vn_enabled, vi) {
                                    (true, true) => "Tiếng Việt (V)",
                                    (true, false) => "Vietnamese (V)",
                                    (false, true) => "Tiếng Anh (E)",
                                    (false, false) => "English (E)",
                                };
                                let status_color = if vn_enabled {
                                    egui::Color32::from_rgb(0, 103, 192)
                                } else {
                                    egui::Color32::from_rgb(100, 100, 100)
                                };
                                ui.label(egui::RichText::new(status_text).color(status_color).strong());
                            },
                            |ui| {
                            egui::Grid::new("vntype_main_grid")
                                .num_columns(2)
                                .spacing([14.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label(if vi { "Bảng mã:" } else { "Character set:" });
                                    let unicode_label = match self.vm.draft.unicode_form {
                                        UnicodeForm::Nfc => {
                                            if vi { "Unicode dựng sẵn (NFC)" } else { "Unicode (NFC)" }
                                        }
                                        UnicodeForm::Nfd => {
                                            if vi { "Unicode tổ hợp (NFD)" } else { "Unicode (NFD)" }
                                        }
                                    };
                                    egui::ComboBox::from_id_salt("unicode_form")
                                        .width(248.0)
                                        .selected_text(unicode_label)
                                        .show_ui(ui, |ui| {
                                            if ui
                                                .selectable_value(
                                                    &mut self.vm.draft.unicode_form,
                                                    UnicodeForm::Nfc,
                                                    if vi {
                                                        "Unicode dựng sẵn (NFC)"
                                                    } else {
                                                        "Unicode (NFC)"
                                                    },
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .selectable_value(
                                                    &mut self.vm.draft.unicode_form,
                                                    UnicodeForm::Nfd,
                                                    if vi {
                                                        "Unicode tổ hợp (NFD)"
                                                    } else {
                                                        "Unicode (NFD)"
                                                    },
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        });
                                    ui.end_row();

                                    ui.label(if vi { "Kiểu gõ:" } else { "Input method:" });
                                    let method_label = match self.vm.draft.input_method.as_str() {
                                        "simple_telex" => {
                                            if vi { "Telex giản lược" } else { "Simple Telex" }
                                        }
                                        "vni" => "VNI",
                                        "viqr" => "VIQR",
                                        _ => "Telex",
                                    };
                                    egui::ComboBox::from_id_salt("input_method")
                                        .width(248.0)
                                        .selected_text(method_label)
                                        .show_ui(ui, |ui| {
                                            let items = [
                                                ("telex", "Telex"),
                                                (
                                                    "simple_telex",
                                                    if vi { "Telex giản lược" } else { "Simple Telex" },
                                                ),
                                                ("vni", "VNI"),
                                                ("viqr", "VIQR"),
                                            ];
                                            for (val, label) in items {
                                                if ui
                                                    .selectable_label(
                                                        self.vm.draft.input_method == val,
                                                        label,
                                                    )
                                                    .clicked()
                                                {
                                                    self.vm.set_input_method(val);
                                                    changed = true;
                                                }
                                            }
                                        });
                                    ui.end_row();

                                    ui.label(if vi { "Phím chuyển:" } else { "Switch key:" });
                                    ui.horizontal(|ui| {
                                        if ui
                                            .radio_value(
                                                &mut self.vm.draft.switch_key,
                                                SwitchKey::AltComma,
                                                "Alt + ,",
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        ui.add_space(12.0);
                                        if ui
                                            .radio_value(
                                                &mut self.vm.draft.switch_key,
                                                SwitchKey::AltZ,
                                                "Alt + Z",
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                    });
                                    ui.end_row();
                                });
                        });

                        let card_h = ctrl_resp.rect.height();
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            let gap = 8.0_f32;
                            ui.spacing_mut().item_spacing.y = gap;
                            let btn_h = ((card_h - 3.0 * gap) / 4.0).max(27.0);
                            let btn_size = egui::vec2(108.0, btn_h);
                            if icon_button(
                                ui,
                                btn_size,
                                ButtonIcon::Check,
                                if vi { "Đóng" } else { "Close" },
                                true,
                            )
                            .clicked()
                            {
                                self.save_and_notify();
                                self.hide_or_close(ctx);
                            }

                            if icon_button(
                                ui,
                                btn_size,
                                ButtonIcon::Exit,
                                if vi { "Kết thúc" } else { "Exit" },
                                false,
                            )
                            .clicked()
                            {
                                self.quit_app(ctx);
                            }

                            if icon_button(
                                ui,
                                btn_size,
                                ButtonIcon::Reset,
                                if vi { "Mặc định" } else { "Default" },
                                false,
                            )
                            .clicked()
                            {
                                self.vm.draft = Config::default();
                                changed = true;
                            }

                            let (expand_icon, expand_text) = match (self.expanded, vi) {
                                (false, true) => (ButtonIcon::Expand, "Mở rộng"),
                                (true, true) => (ButtonIcon::Collapse, "Thu nhỏ"),
                                (false, false) => (ButtonIcon::Expand, "Expand"),
                                (true, false) => (ButtonIcon::Collapse, "Contract"),
                            };
                            if icon_button(ui, btn_size, expand_icon, expand_text, false).clicked() {
                                self.expanded = !self.expanded;
                                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                                    WINDOW_W,
                                    self.target_height(),
                                )));
                            }
                        });
                    });

                    if self.expanded {
                        ui.add_space(4.0);
                        section_card(
                            ui,
                            if vi { "Tuỳ chọn khác" } else { "Advanced options" },
                            CONTENT_W,
                            |ui| {
                                egui::Grid::new("vntype_adv_grid")
                                    .num_columns(2)
                                    .spacing([28.0, 6.0])
                                    .show(ui, |ui| {
                                        if ui
                                            .checkbox(
                                                &mut self.vm.draft.w_as_u,
                                                if vi {
                                                    "Cho phép gõ tự do (w → ư)"
                                                } else {
                                                    "Allow free w → ư typing"
                                                },
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        if ui
                                            .checkbox(
                                                &mut self.vm.draft.spell_check,
                                                if vi {
                                                    "Bật kiểm tra chính tả"
                                                } else {
                                                    "Enable spell check"
                                                },
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        ui.end_row();

                                        let mut tone_old = self.vm.draft.tone_style == ToneStyle::Old;
                                        if ui
                                            .checkbox(
                                                &mut tone_old,
                                                if vi {
                                                    "Đặt dấu oà, uý (thay vì òa, úy)"
                                                } else {
                                                    "Use oà, uý (instead of òa, úy)"
                                                },
                                            )
                                            .changed()
                                        {
                                            self.vm.set_tone_style(if tone_old {
                                                ToneStyle::Old
                                            } else {
                                                ToneStyle::New
                                            });
                                            changed = true;
                                        }
                                        if ui
                                            .checkbox(
                                                &mut self.vm.draft.restore_after_space,
                                                if vi {
                                                    "Khôi phục từ sau dấu cách"
                                                } else {
                                                    "Restore word after Space"
                                                },
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        ui.end_row();

                                        if ui
                                            .checkbox(
                                                &mut self.vm.draft.edit_previous_word,
                                                if vi {
                                                    "Nạp lại từ trước con trỏ"
                                                } else {
                                                    "Edit word before caret"
                                                },
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        if ui
                                            .checkbox(
                                                &mut self.vm.draft.auto_capitalize,
                                                if vi {
                                                    "Tự động viết hoa đầu câu"
                                                } else {
                                                    "Auto-capitalize sentences"
                                                },
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        ui.end_row();
                                    });
                            },
                        );

                        ui.add_space(4.0);
                        ui.horizontal_top(|ui| {
                            section_card(
                                ui,
                                if vi { "Tuỳ chọn gõ tắt" } else { "Macro options" },
                                250.0,
                                |ui| {
                                    if ui
                                        .checkbox(
                                            &mut self.vm.draft.macro_enabled,
                                            if vi { "Cho phép gõ tắt" } else { "Enable macro" },
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    if ui
                                        .checkbox(
                                            &mut self.vm.draft.macro_in_off_mode,
                                            if vi {
                                                "Gõ tắt cả khi tắt tiếng Việt"
                                            } else {
                                                "Allow macro in off mode"
                                            },
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                },
                            );

                            ui.add_space(4.0);
                            section_card(
                                ui,
                                if vi { "Hệ thống" } else { "System settings" },
                                250.0,
                                |ui| {
                                    if ui
                                        .checkbox(
                                            &mut self.vm.draft.default_vietnamese,
                                            if vi {
                                                "Bật Tiếng Việt mặc định"
                                            } else {
                                                "Enable Vietnamese by default"
                                            },
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    ui.checkbox(&mut self.vietnamese_ui, "Giao diện Tiếng Việt");
                                },
                            );
                        });
                    }
                });

            if changed {
                self.save_and_notify();
            }
        }
    }

    fn build_native_options() -> eframe::NativeOptions {
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([WINDOW_W, COMPACT_H])
                .with_resizable(false)
                .with_maximize_button(false)
                .with_icon(Arc::new(build_app_icon())),
            ..Default::default()
        }
    }

    /// Khởi chạy cửa sổ giao diện cấu hình VNType ở chế độ độc lập.
    pub fn run_settings_gui(vm: SettingsViewModel) {
        let _ = eframe::run_native(
            concat!("VNType ", env!("CARGO_PKG_VERSION")),
            build_native_options(),
            Box::new(move |cc| Ok(Box::new(VntypeSettingsApp::new(cc, vm, None, None, None, None)))),
        );
    }

    /// Khởi chạy cửa sổ giao diện cấu hình nhúng trên luồng chính cùng với `vntype-hook`.
    pub fn run_embedded_gui(
        config_path: PathBuf,
        bridge: Arc<HookGuiBridge>,
        on_toggle: Box<dyn Fn() + Send>,
        on_config_changed: Box<dyn Fn() + Send>,
        on_quit: Box<dyn Fn() + Send>,
    ) {
        let vm = SettingsViewModel::load(config_path);
        let _ = eframe::run_native(
            concat!("VNType ", env!("CARGO_PKG_VERSION")),
            build_native_options(),
            Box::new(move |cc| {
                Ok(Box::new(VntypeSettingsApp::new(
                    cc,
                    vm,
                    Some(bridge),
                    Some(on_toggle),
                    Some(on_config_changed),
                    Some(on_quit),
                )))
            }),
        );
    }
}
