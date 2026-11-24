//! Giao diện cài đặt native trên Linux (GTK3) theo bố cục một trang gọn gàng.

use std::cell::RefCell;
use std::process::Stdio;
use std::rc::Rc;

use gtk::gio::prelude::*;
use gtk::prelude::*;
use vntype_config::{ToneStyle, default_config_path};
use vntype_core::{CHARSETS, charset_display_name};
use vntype_settings_core::{PlatformCapabilities, SettingsViewModel};

const INPUT_METHODS: &[(&str, &str)] =
    &[("telex", "Telex"), ("simple_telex", "Simple Telex"), ("vni", "VNI"), ("viqr", "VIQR")];

fn print_help() {
    println!(
        "\
vntype-linux-settings {} — Giao diện cài đặt VNType trên Linux

Cách dùng:
  vntype-linux-settings [TUỲ CHỌN]

Tuỳ chọn:
  --print        In cấu hình hiện tại dưới dạng JSON
  -h, --help     Hiển thị trợ giúp
  -V, --version  Hiển thị phiên bản",
        env!("CARGO_PKG_VERSION")
    );
}

fn frame_with_content(title: &str, child: &impl IsA<gtk::Widget>) -> gtk::Frame {
    let frame = gtk::Frame::new(Some(title));
    frame.set_shadow_type(gtk::ShadowType::EtchedIn);
    frame.add(child);
    frame
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("vntype-linux-settings {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let config_path = default_config_path();
    let vm = SettingsViewModel::load(config_path);
    if args.iter().any(|a| a == "--print") {
        println!("{}", serde_json::to_string_pretty(&vm.draft).unwrap_or_default());
        return;
    }

    let app = gtk::Application::new(Some("dev.hephaestus.vntype.settings"), Default::default());
    let vm_cell = Rc::new(RefCell::new(vm));

    app.connect_activate(move |app| {
        build_ui(app, Rc::clone(&vm_cell));
    });
    app.run();
}

fn build_ui(app: &gtk::Application, vm_cell: Rc<RefCell<SettingsViewModel>>) {
    let caps = PlatformCapabilities::linux();

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(format!("VNType {}", env!("CARGO_PKG_VERSION")))
        .icon_name("vntype")
        .default_width(600)
        .resizable(false)
        .border_width(12)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 10);

    // --- Phần trên: Khung "Điều khiển" toàn chiều ngang ---
    let ctrl_grid = gtk::Grid::builder().row_spacing(8).column_spacing(12).border_width(10).build();
    let charset_combo = gtk::ComboBoxText::new();
    for &key in CHARSETS {
        charset_combo.append(Some(key), charset_display_name(key).unwrap_or(key));
    }
    charset_combo.set_hexpand(true);

    let im_combo = gtk::ComboBoxText::new();
    for (id, label) in INPUT_METHODS {
        im_combo.append(Some(id), label);
    }
    im_combo.set_hexpand(true);

    let tone_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let tone_new = gtk::RadioButton::with_label("Mới (hoà, thuý)");
    let tone_old = gtk::RadioButton::with_label_from_widget(&tone_new, "Cũ (hòa, thúy)");
    tone_box.pack_start(&tone_new, false, false, 0);
    tone_box.pack_start(&tone_old, false, false, 0);

    ctrl_grid.attach(
        &gtk::Label::builder().label("Bảng mã:").halign(gtk::Align::Start).build(),
        0,
        0,
        1,
        1,
    );
    ctrl_grid.attach(&charset_combo, 1, 0, 1, 1);
    ctrl_grid.attach(
        &gtk::Label::builder().label("Kiểu gõ:").halign(gtk::Align::Start).build(),
        0,
        1,
        1,
        1,
    );
    ctrl_grid.attach(&im_combo, 1, 1, 1, 1);
    ctrl_grid.attach(
        &gtk::Label::builder().label("Đặt dấu:").halign(gtk::Align::Start).build(),
        0,
        2,
        1,
        1,
    );
    ctrl_grid.attach(&tone_box, 1, 2, 1, 1);

    let ctrl_frame = frame_with_content("Điều khiển", &ctrl_grid);
    root.pack_start(&ctrl_frame, false, false, 0);

    // --- Phần giữa: Khung "Tùy chọn khác" (2 cột checkbox) ---
    let opt_grid = gtk::Grid::builder()
        .row_spacing(6)
        .column_spacing(18)
        .border_width(10)
        .column_homogeneous(true)
        .build();

    let chk_default_vi = gtk::CheckButton::with_label("Bật tiếng Việt mặc định");
    let chk_spell = gtk::CheckButton::with_label("Bật kiểm tra chính tả");
    let chk_w_as_u = gtk::CheckButton::with_label("Phím w đứng riêng thành ư (Telex)");
    let chk_brackets = gtk::CheckButton::with_label("Phím [ / ] thành ơ / ư (Telex)");
    let chk_quick_telex = gtk::CheckButton::with_label("Quick Telex (cc=ch, gg=gi...)");
    let chk_coda_k = gtk::CheckButton::with_label("Âm cuối k với sắc/nặng (Đắk Lắk)");
    let chk_edit_prev = gtk::CheckButton::with_label("Sửa từ trước bằng Backspace");
    let chk_auto_cap = gtk::CheckButton::with_label("Tự viết hoa đầu câu");
    let chk_smart_caps = gtk::CheckButton::with_label("Giữ chữ hoa 2 chữ cái đầu");

    let left_opts = [&chk_default_vi, &chk_spell, &chk_w_as_u, &chk_brackets, &chk_quick_telex];
    let right_opts = [&chk_coda_k, &chk_edit_prev, &chk_auto_cap, &chk_smart_caps];

    for (i, chk) in left_opts.iter().enumerate() {
        opt_grid.attach(*chk, 0, i as i32, 1, 1);
    }
    for (i, chk) in right_opts.iter().enumerate() {
        opt_grid.attach(*chk, 1, i as i32, 1, 1);
    }
    root.pack_start(&frame_with_content("Tùy chọn khác", &opt_grid), false, false, 0);

    // --- Phần dưới: Khung "Tùy chọn gõ tắt" ---
    let macro_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    macro_box.set_border_width(10);

    let macro_grid =
        gtk::Grid::builder().row_spacing(6).column_spacing(18).column_homogeneous(true).build();
    let chk_macro_enabled = gtk::CheckButton::with_label("Cho phép gõ tắt");
    let chk_macro_undo = gtk::CheckButton::with_label("Backspace hoàn tác gõ tắt");
    let chk_macro_digits = gtk::CheckButton::with_label("Cho phép chữ số trong gõ tắt");

    macro_grid.attach(&chk_macro_enabled, 0, 0, 1, 1);
    macro_grid.attach(&chk_macro_undo, 1, 0, 1, 1);
    macro_grid.attach(&chk_macro_digits, 0, 1, 1, 1);
    macro_box.pack_start(&macro_grid, false, false, 0);

    if caps.supports_macro_file_editor {
        let macro_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let btn_open_macros = gtk::Button::with_label("Bảng gõ tắt (macros.txt)...");
        let macro_count_label = gtk::Label::builder().halign(gtk::Align::Start).build();
        macro_row.pack_start(&btn_open_macros, false, false, 0);
        macro_row.pack_start(&macro_count_label, true, true, 0);
        macro_box.pack_start(&macro_row, false, false, 0);

        let update_macro_count = {
            let vm_cell = Rc::clone(&vm_cell);
            let macro_count_label = macro_count_label.clone();
            move || {
                let vm = vm_cell.borrow();
                macro_count_label.set_text(&format!("({} quy tắc)", vm.macro_entry_count()));
            }
        };
        update_macro_count();

        let vm_for_open = Rc::clone(&vm_cell);
        btn_open_macros.connect_clicked(move |_| {
            if let Ok(path) = vm_for_open.borrow().ensure_macros_file() {
                let _ = std::process::Command::new("xdg-open")
                    .arg(path)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
                update_macro_count();
            }
        });
    }

    root.pack_start(&frame_with_content("Tùy chọn gõ tắt", &macro_box), false, false, 0);

    // --- Thanh chân trang ---
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let btn_info = gtk::Button::with_label("Thông tin");
    let btn_defaults = gtk::Button::with_label("Mặc định");
    let btn_revert = gtk::Button::with_label("Hoàn tác");
    let btn_save_close = gtk::Button::with_label("Lưu & Đóng");
    btn_save_close.style_context().add_class("suggested-action");

    footer.pack_start(&btn_info, false, false, 0);
    footer.pack_end(&btn_save_close, false, false, 0);
    footer.pack_end(&btn_revert, false, false, 0);
    footer.pack_end(&btn_defaults, false, false, 0);
    root.pack_end(&footer, false, false, 0);

    let syncing = Rc::new(RefCell::new(false));

    let sync_ui_from_vm = {
        let vm_cell = Rc::clone(&vm_cell);
        let syncing = Rc::clone(&syncing);
        let im_combo = im_combo.clone();
        let charset_combo = charset_combo.clone();
        let tone_new = tone_new.clone();
        let tone_old = tone_old.clone();
        let chk_default_vi = chk_default_vi.clone();
        let chk_spell = chk_spell.clone();
        let chk_w_as_u = chk_w_as_u.clone();
        let chk_brackets = chk_brackets.clone();
        let chk_quick_telex = chk_quick_telex.clone();
        let chk_coda_k = chk_coda_k.clone();
        let chk_edit_prev = chk_edit_prev.clone();
        let chk_auto_cap = chk_auto_cap.clone();
        let chk_smart_caps = chk_smart_caps.clone();
        let chk_macro_enabled = chk_macro_enabled.clone();
        let chk_macro_undo = chk_macro_undo.clone();
        let chk_macro_digits = chk_macro_digits.clone();
        let btn_revert = btn_revert.clone();

        move || {
            *syncing.borrow_mut() = true;
            let vm = vm_cell.borrow();
            let d = &vm.draft;

            im_combo.set_active_id(Some(&d.input_method));
            charset_combo.set_active_id(Some(&d.charset));
            match d.tone_style {
                ToneStyle::New => tone_new.set_active(true),
                ToneStyle::Old => tone_old.set_active(true),
            }

            chk_default_vi.set_active(d.default_vietnamese);
            chk_spell.set_active(d.spell_check);
            chk_w_as_u.set_active(d.w_as_u);
            chk_brackets.set_active(d.bracket_keys);
            chk_quick_telex.set_active(d.quick_telex);
            chk_coda_k.set_active(d.allow_coda_k);
            chk_edit_prev.set_active(d.edit_previous_word);
            chk_auto_cap.set_active(d.auto_capitalize);
            chk_smart_caps.set_active(d.smart_caps);
            chk_macro_enabled.set_active(d.macro_enabled);
            chk_macro_undo.set_active(d.macro_undo);
            chk_macro_digits.set_active(d.macro_digits);

            btn_revert.set_sensitive(vm.is_dirty());
            *syncing.borrow_mut() = false;
        }
    };

    let sync_vm_from_ui = {
        let vm_cell = Rc::clone(&vm_cell);
        let syncing = Rc::clone(&syncing);
        let im_combo = im_combo.clone();
        let charset_combo = charset_combo.clone();
        let tone_new = tone_new.clone();
        let chk_default_vi = chk_default_vi.clone();
        let chk_spell = chk_spell.clone();
        let chk_w_as_u = chk_w_as_u.clone();
        let chk_brackets = chk_brackets.clone();
        let chk_quick_telex = chk_quick_telex.clone();
        let chk_coda_k = chk_coda_k.clone();
        let chk_edit_prev = chk_edit_prev.clone();
        let chk_auto_cap = chk_auto_cap.clone();
        let chk_smart_caps = chk_smart_caps.clone();
        let chk_macro_enabled = chk_macro_enabled.clone();
        let chk_macro_undo = chk_macro_undo.clone();
        let chk_macro_digits = chk_macro_digits.clone();
        let btn_revert = btn_revert.clone();

        move || {
            if *syncing.borrow() {
                return;
            }
            let mut vm = vm_cell.borrow_mut();
            if let Some(id) = im_combo.active_id() {
                vm.set_input_method(id.as_str());
            }
            if let Some(id) = charset_combo.active_id() {
                vm.set_charset(id.as_str());
            }
            vm.set_tone_style(if tone_new.is_active() { ToneStyle::New } else { ToneStyle::Old });

            vm.draft.default_vietnamese = chk_default_vi.is_active();
            vm.draft.spell_check = chk_spell.is_active();
            vm.draft.w_as_u = chk_w_as_u.is_active();
            vm.draft.bracket_keys = chk_brackets.is_active();
            vm.draft.quick_telex = chk_quick_telex.is_active();
            vm.draft.allow_coda_k = chk_coda_k.is_active();
            vm.draft.edit_previous_word = chk_edit_prev.is_active();
            vm.draft.auto_capitalize = chk_auto_cap.is_active();
            vm.draft.smart_caps = chk_smart_caps.is_active();
            vm.draft.macro_enabled = chk_macro_enabled.is_active();
            vm.draft.macro_undo = chk_macro_undo.is_active();
            vm.draft.macro_digits = chk_macro_digits.is_active();

            btn_revert.set_sensitive(vm.is_dirty());
        }
    };

    sync_ui_from_vm();

    for combo in [&im_combo, &charset_combo] {
        let f = sync_vm_from_ui.clone();
        combo.connect_changed(move |_| f());
    }
    for radio in [&tone_new, &tone_old] {
        let f = sync_vm_from_ui.clone();
        radio.connect_toggled(move |_| f());
    }
    for chk in [
        &chk_default_vi,
        &chk_spell,
        &chk_w_as_u,
        &chk_brackets,
        &chk_quick_telex,
        &chk_coda_k,
        &chk_edit_prev,
        &chk_auto_cap,
        &chk_smart_caps,
        &chk_macro_enabled,
        &chk_macro_undo,
        &chk_macro_digits,
    ] {
        let f = sync_vm_from_ui.clone();
        chk.connect_toggled(move |_| f());
    }

    let sync_ui_for_defaults = sync_ui_from_vm.clone();
    let vm_for_defaults = Rc::clone(&vm_cell);
    btn_defaults.connect_clicked(move |_| {
        vm_for_defaults.borrow_mut().reset_to_defaults();
        sync_ui_for_defaults();
    });

    let sync_ui_for_revert = sync_ui_from_vm.clone();
    let vm_for_revert = Rc::clone(&vm_cell);
    btn_revert.connect_clicked(move |_| {
        vm_for_revert.borrow_mut().reset_draft_to_saved();
        sync_ui_for_revert();
    });

    let vm_for_save = Rc::clone(&vm_cell);
    let win_for_save = window.clone();
    btn_save_close.connect_clicked(move |_| {
        sync_vm_from_ui();
        match vm_for_save.borrow_mut().apply_and_save() {
            Ok(()) => win_for_save.close(),
            Err(err) => {
                let dialog = gtk::MessageDialog::new(
                    Some(&win_for_save),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Error,
                    gtk::ButtonsType::Ok,
                    &format!("Không thể lưu cấu hình: {err}"),
                );
                dialog.run();
                dialog.close();
            }
        }
    });

    let win_for_info = window.clone();
    let vm_for_info = Rc::clone(&vm_cell);
    btn_info.connect_clicked(move |_| {
        let vm = vm_for_info.borrow();
        let dialog = gtk::MessageDialog::new(
            Some(&win_for_info),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Info,
            gtk::ButtonsType::Ok,
            &format!(
                "VNType {} — Bộ gõ tiếng Việt đa nền tảng\n\n\
                 Cấu hình: {}\n\
                 Gõ tắt: {}\n\n\
                 Chuyển Anh/Việt trên Linux qua phím tắt Input Source hệ thống (Super + Space).",
                env!("CARGO_PKG_VERSION"),
                vm.config_path.display(),
                vm.macros_path().display()
            ),
        );
        dialog.run();
        dialog.close();
    });

    window.add(&root);
    window.show_all();
}

#[cfg(test)]
mod tests {
    use super::*;
    use vntype_core::InputMethodSpec;

    #[test]
    fn input_methods_match_core_specs() {
        for (id, _) in INPUT_METHODS {
            assert!(InputMethodSpec::by_name(id).is_some());
        }
    }
}
