//! Giao diện nhị phân C ABI chuẩn của VNType (`vntype-ffi`).
//!
//! Mọi hàm xuất ra trong mô-đun này đều khớp 1-1 với tệp `include/vntype.h`, bắt mọi ngoại lệ
//! bằng `catch_unwind` và trả về chuỗi hiệu ứng nguyên tử [`VntEffectList`].

#![allow(clippy::missing_safety_doc)]

pub mod c_types;
pub mod handle;

use std::ffi::{CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;
use std::sync::OnceLock;

use vntype_adapter::{
    AdapterController, char_to_byte_offset, char_to_utf16_offset, is_word_codepoint, utf16_to_byte_offset,
};
use vntype_core::{CHARSETS, charset};
use vntype_engine::{Config, Key, KeyEvent, OutputMode};

pub use c_types::*;
use handle::{VntController, field_kind_from_u32, guard, opt_cstr, with_controller};

/// Trả về phiên bản thư viện hiện hành dạng chuỗi C tĩnh.
///
/// # Examples
///
/// ```
/// use std::ffi::CStr;
///
/// let ver = unsafe { CStr::from_ptr(vntype_ffi::vnt_version()) }.to_str().unwrap();
/// assert!(!ver.is_empty());
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn vnt_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Tạo một thực thể [`VntController`] mới với chuỗi cấu hình JSON (có thể NULL) và chế độ xuất chữ `mode`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_new(config_json: *const c_char, mode: u32) -> *mut VntController {
    guard(ptr::null_mut(), || {
        let config: Config = match unsafe { opt_cstr(config_json) } {
            None => Config::default(),
            Some(s) if s.trim().is_empty() => Config::default(),
            Some(s) => match serde_json::from_str(&s) {
                Ok(c) => c,
                Err(_) => return ptr::null_mut(),
            },
        };
        let out_mode = if mode == VNT_MODE_REPLACE { OutputMode::Replace } else { OutputMode::Preedit };
        let inner = AdapterController::new(config, out_mode);
        Box::into_raw(Box::new(VntController::new(inner)))
    })
}

/// Giải phóng một thực thể [`VntController`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_free(c: *mut VntController) {
    if !c.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(c) })));
    }
}

/// Bật hoặc tắt chế độ tách thao tác `Replace` thành giao dịch bất đồng bộ có `VNT_EFFECT_WAIT_MS`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_set_async_replace(c: *mut VntController, enabled: bool) {
    unsafe { with_controller(c, (), |ctrl| ctrl.inner.set_async_replace(enabled)) };
}

/// Bật hoặc tắt chế độ cho phép `Replace` trực tiếp không cần xác minh văn bản xung quanh.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_set_allow_unverified_replace(c: *mut VntController, allow: bool) {
    unsafe { with_controller(c, (), |ctrl| ctrl.inner.set_allow_unverified_replace(allow)) };
}

/// Xử lý một phím theo tên chuỗi UTF-8 ("a", "Backspace", "Enter", "Escape", ...) và trả về [`VntEffectList`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_process_key(
    c: *mut VntController,
    key: *const c_char,
    ctrl: bool,
    alt: bool,
    meta: bool,
    raw_keycode: u32,
) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let Some(name) = opt_cstr(key) else { return VntEffectList::EMPTY };
            let ev = KeyEvent { key: Key::parse(&name), ctrl, alt, meta };
            let effects = controller.inner.process_key(ev, raw_keycode);
            controller.pack_effects(effects)
        })
    }
}

/// Xử lý một phím theo mã điểm Unicode (`codepoint`) và trả về [`VntEffectList`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_process_char(
    c: *mut VntController,
    codepoint: u32,
    ctrl: bool,
    alt: bool,
    meta: bool,
    raw_keycode: u32,
) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let Some(ch) = char::from_u32(codepoint) else { return VntEffectList::EMPTY };
            let ev = KeyEvent { key: Key::Char(ch), ctrl, alt, meta };
            let effects = controller.inner.process_key(ev, raw_keycode);
            controller.pack_effects(effects)
        })
    }
}

/// Thực thi bước tiếp theo khi bộ hẹn giờ `VNT_EFFECT_WAIT_MS` hoàn tất.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_on_timer_tick(c: *mut VntController) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let effects = controller.inner.on_timer_tick();
            controller.pack_effects(effects)
        })
    }
}

/// Thông báo ứng dụng nhận tiêu điểm thay đổi (`app_id` có thể NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_focus_changed(
    c: *mut VntController,
    app_id: *const c_char,
) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let app = opt_cstr(app_id);
            let effects = controller.inner.focus_changed(app.as_deref());
            controller.pack_effects(effects)
        })
    }
}

/// Cập nhật ảnh chụp nhanh văn bản xung quanh con trỏ với toạ độ theo đơn vị mã UTF-16.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_set_surrounding_utf16(
    c: *mut VntController,
    text: *const c_char,
    cursor_utf16: u32,
    anchor_utf16: u32,
) {
    unsafe {
        with_controller(c, (), |controller| {
            let sur = opt_cstr(text);
            controller.inner.set_surrounding_utf16(sur.as_deref(), cursor_utf16, anchor_utf16);
        });
    }
}

/// Cập nhật chuỗi văn bản nằm ngay trước con trỏ.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_set_surrounding_before(
    c: *mut VntController,
    before_cursor: *const c_char,
) {
    unsafe {
        with_controller(c, (), |controller| {
            let before = opt_cstr(before_cursor);
            controller.inner.set_surrounding_before_only(before.as_deref());
        });
    }
}

/// Cập nhật phân loại trường nhập liệu (`VNT_FIELD_*`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_set_field_kind(
    c: *mut VntController,
    field_kind: u32,
) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let kind = field_kind_from_u32(field_kind);
            let effects = controller.inner.set_field_kind(kind);
            controller.pack_effects(effects)
        })
    }
}

/// Thông báo con trỏ soạn thảo đã bị dịch chuyển.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_cursor_moved(c: *mut VntController) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let effects = controller.inner.cursor_moved();
            controller.pack_effects(effects)
        })
    }
}

/// Đảo trạng thái bật/tắt gõ tiếng Việt.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_toggle_vietnamese(c: *mut VntController) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let effects = controller.inner.toggle_vietnamese();
            controller.pack_effects(effects)
        })
    }
}

/// Chốt vùng văn bản đang soạn dở.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_flush(c: *mut VntController) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let effects = controller.inner.flush();
            controller.pack_effects(effects)
        })
    }
}

/// Xoá sạch trạng thái từ đang gõ và hàng đợi mà không phát sinh hiệu ứng.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_reset(c: *mut VntController) {
    unsafe { with_controller(c, (), |controller| controller.inner.reset()) };
}

/// Thay đổi chế độ xuất chữ (`VNT_MODE_PREEDIT` hoặc `VNT_MODE_REPLACE`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_set_output_mode(c: *mut VntController, mode: u32) -> VntEffectList {
    unsafe {
        with_controller(c, VntEffectList::EMPTY, |controller| {
            let out_mode = if mode == VNT_MODE_REPLACE { OutputMode::Replace } else { OutputMode::Preedit };
            let effects = controller.inner.set_output_mode(out_mode);
            controller.pack_effects(effects)
        })
    }
}

/// Kiểm tra xem bộ gõ tiếng Việt có đang bật hay không.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_is_enabled(c: *mut VntController) -> bool {
    unsafe { with_controller(c, false, |controller| controller.inner.is_enabled()) }
}

/// Kiểm tra xem bộ điều khiển có đang bận trong giao dịch bất đồng bộ hay không.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_is_busy(c: *mut VntController) -> bool {
    unsafe { with_controller(c, false, |controller| controller.inner.is_busy()) }
}

/// Trả về chuỗi văn bản mà engine tin là đang hiển thị cho từ hiện tại.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_display_text(c: *mut VntController) -> *const c_char {
    unsafe {
        with_controller(c, ptr::null(), |controller| {
            let disp = controller.inner.display_text().replace('\0', "");
            controller.display_buf = CString::new(disp).unwrap_or_default();
            controller.display_buf.as_ptr()
        })
    }
}

/// Áp dụng một mảnh JSON cấu hình mới lên bộ điều khiển và trả về các hiệu ứng cần thực thi.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_apply_config_json(
    c: *mut VntController,
    json: *const c_char,
    out_effects: *mut VntEffectList,
) -> bool {
    unsafe {
        with_controller(c, false, |controller| {
            let Some(text) = opt_cstr(json) else { return false };
            let Some(effects) = controller.inner.apply_config_json(&text) else { return false };
            let list = controller.pack_effects(effects);
            if !out_effects.is_null() {
                *out_effects = list;
            }
            true
        })
    }
}

/// Trả về chuỗi JSON cấu hình hiện hành của phiên.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_config_json(c: *mut VntController) -> *const c_char {
    unsafe {
        with_controller(c, ptr::null(), |controller| {
            let j = serde_json::to_string(controller.inner.session().config()).unwrap_or_default();
            controller.json_buf = CString::new(j).unwrap_or_default();
            controller.json_buf.as_ptr()
        })
    }
}

/// Trả về số thứ tự phiên bản cấu hình nội bộ.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_config_version(c: *mut VntController) -> u64 {
    unsafe { with_controller(c, 0, |controller| controller.inner.config_version()) }
}

/// Trả về chuỗi JSON của `AppProfile` hiện hành.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_app_profile_json(c: *mut VntController) -> *const c_char {
    unsafe {
        with_controller(c, ptr::null(), |controller| {
            let j = serde_json::to_string(controller.inner.session().profile()).unwrap_or_default();
            controller.json_buf = CString::new(j).unwrap_or_default();
            controller.json_buf.as_ptr()
        })
    }
}

/// Trả về chế độ xuất chữ gợi ý từ `AppProfile`: 0 = không đặt, 1 = PREEDIT, 2 = REPLACE.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_profile_output_mode(c: *mut VntController) -> u32 {
    unsafe {
        with_controller(c, 0, |controller| match controller.inner.session().profile().output_mode {
            Some(OutputMode::Preedit) => 1,
            Some(OutputMode::Replace) => 2,
            None => 0,
        })
    }
}

/// Thêm danh sách từ vào từ điển của phiên hiện tại.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_add_dictionary_words(
    c: *mut VntController,
    words: *const c_char,
    vietnamese: bool,
) {
    unsafe {
        with_controller(c, (), |controller| {
            let Some(text) = opt_cstr(words) else { return };
            controller.inner.add_dictionary_words(&text, vietnamese);
        });
    }
}

/// Nạp mới danh sách từ điển tiếng Anh hoặc tiếng Việt cho phiên hiện tại.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_controller_load_dictionary(
    c: *mut VntController,
    english: *const c_char,
    vietnamese: *const c_char,
) {
    unsafe {
        with_controller(c, (), |controller| {
            let en = opt_cstr(english);
            let vi = opt_cstr(vietnamese);
            controller.inner.load_dictionary(en.as_deref(), vi.as_deref());
        });
    }
}

/// Đổi chỉ số ký tự (`char`) sang chỉ số đơn vị mã UTF-16 (`UINT32_MAX` nếu lỗi).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_char_to_utf16_offset(text: *const c_char, chars: u32) -> u32 {
    guard(u32::MAX, || {
        let Some(text) = (unsafe { opt_cstr(text) }) else { return u32::MAX };
        char_to_utf16_offset(&text, chars).unwrap_or(u32::MAX)
    })
}

/// Đổi chỉ số ký tự (`char`) sang chỉ số byte UTF-8 (`UINT32_MAX` nếu lỗi).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_char_to_byte_offset(text: *const c_char, chars: u32) -> u32 {
    guard(u32::MAX, || {
        let Some(text) = (unsafe { opt_cstr(text) }) else { return u32::MAX };
        char_to_byte_offset(&text, chars).map(|v| v as u32).unwrap_or(u32::MAX)
    })
}

/// Đổi chỉ số đơn vị mã UTF-16 sang chỉ số byte UTF-8 (`UINT32_MAX` nếu lỗi).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_utf16_to_byte_offset(text: *const c_char, units: u32) -> u32 {
    guard(u32::MAX, || {
        let Some(text) = (unsafe { opt_cstr(text) }) else { return u32::MAX };
        utf16_to_byte_offset(&text, units).map(|v| v as u32).unwrap_or(u32::MAX)
    })
}

/// Kiểm tra một mã điểm Unicode có phải là ký tự thuộc từ hay không.
///
/// # Examples
///
/// ```
/// assert!(vntype_ffi::vnt_is_word_codepoint('ế' as u32));
/// assert!(!vntype_ffi::vnt_is_word_codepoint(' ' as u32));
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn vnt_is_word_codepoint(codepoint: u32) -> bool {
    char::from_u32(codepoint).is_some_and(is_word_codepoint)
}

/// Trả về danh sách các bảng mã được hỗ trợ, mỗi dòng có định dạng `"key\tTên hiển thị\n"`.
#[unsafe(no_mangle)]
pub extern "C" fn vnt_charset_list() -> *const c_char {
    static LIST: OnceLock<CString> = OnceLock::new();
    let list = LIST.get_or_init(|| {
        let mut s = String::new();
        for key in CHARSETS {
            s.push_str(key);
            s.push('\t');
            s.push_str(charset::display_name(key).unwrap_or(key));
            s.push('\n');
        }
        CString::new(s).unwrap_or_default()
    });
    list.as_ptr()
}

/// Mã hoá văn bản từ Unicode sang bảng mã đích.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_charset_encode(
    c: *mut VntController,
    charset_name: *const c_char,
    text: *const c_char,
) -> *const c_char {
    unsafe {
        with_controller(c, ptr::null(), |controller| {
            let Some(name) = opt_cstr(charset_name) else { return ptr::null() };
            let Some(text) = opt_cstr(text) else { return ptr::null() };
            let out = charset::encode(&text, &name);
            controller.charset_buf = CString::new(out).unwrap_or_default();
            controller.charset_buf.as_ptr()
        })
    }
}

/// Giải mã văn bản từ bảng mã nguồn về chuẩn Unicode NFC.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_charset_decode(
    c: *mut VntController,
    charset_name: *const c_char,
    text: *const c_char,
) -> *const c_char {
    unsafe {
        with_controller(c, ptr::null(), |controller| {
            let Some(name) = opt_cstr(charset_name) else { return ptr::null() };
            let Some(text) = opt_cstr(text) else { return ptr::null() };
            let out = charset::decode(&text, &name);
            controller.charset_buf = CString::new(out).unwrap_or_default();
            controller.charset_buf.as_ptr()
        })
    }
}

/// Mã hoá văn bản từ Unicode sang mảng byte thô cho bảng mã một byte.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_charset_encode_bytes(
    c: *mut VntController,
    charset_name: *const c_char,
    text: *const c_char,
    out_len: *mut usize,
) -> *const u8 {
    unsafe {
        with_controller(c, ptr::null(), |controller| {
            if !out_len.is_null() {
                *out_len = 0;
            }
            let Some(name) = opt_cstr(charset_name) else { return ptr::null() };
            let Some(text) = opt_cstr(text) else { return ptr::null() };
            let Some(bytes) = charset::encode_bytes(&text, &name) else { return ptr::null() };
            controller.bytes_buf = bytes;
            if !out_len.is_null() {
                *out_len = controller.bytes_buf.len();
            }
            controller.bytes_buf.as_ptr()
        })
    }
}

/// Giải mã mảng byte thô từ bảng mã một byte về chuỗi Unicode NFC.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vnt_charset_decode_bytes(
    c: *mut VntController,
    charset_name: *const c_char,
    data: *const u8,
    len: usize,
) -> *const c_char {
    unsafe {
        with_controller(c, ptr::null(), |controller| {
            if data.is_null() && len > 0 {
                return ptr::null();
            }
            let Some(name) = opt_cstr(charset_name) else { return ptr::null() };
            let bytes = if len == 0 { &[] } else { slice::from_raw_parts(data, len) };
            let out = charset::decode_bytes(bytes, &name);
            controller.charset_buf = CString::new(out).unwrap_or_default();
            controller.charset_buf.as_ptr()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn ffi_effect_pipeline_roundtrip() {
        unsafe {
            let ctrl = vnt_controller_new(ptr::null(), VNT_MODE_PREEDIT);
            assert!(!ctrl.is_null());

            for ch in ["t\0", "i\0", "e\0", "e\0", "n\0", "g\0", "s\0"] {
                let list =
                    vnt_controller_process_key(ctrl, ch.as_ptr() as *const c_char, false, false, false, 10);
                assert_eq!(list.count, 1);
                let eff = &*list.effects;
                assert_eq!(eff.kind, VNT_EFFECT_SET_PREEDIT);
            }

            let disp = CStr::from_ptr(vnt_controller_display_text(ctrl)).to_str().unwrap();
            assert_eq!(disp, "tiếng");

            let space = " \0";
            let list =
                vnt_controller_process_key(ctrl, space.as_ptr() as *const c_char, false, false, false, 57);
            assert_eq!(list.count, 2);
            let eff0 = &*list.effects;
            let eff1 = &*list.effects.add(1);
            assert_eq!(eff0.kind, VNT_EFFECT_COMMIT_TEXT);
            assert_eq!(CStr::from_ptr(eff0.text).to_str().unwrap(), "tiếng ");
            assert_eq!(eff1.kind, VNT_EFFECT_CLEAR_PREEDIT);

            vnt_controller_free(ctrl);
        }
    }

    #[test]
    fn ffi_config_offsets_and_charsets() {
        unsafe {
            assert!(vnt_controller_new(c"{bad".as_ptr(), VNT_MODE_PREEDIT).is_null());
            vnt_controller_free(ptr::null_mut());

            let ctrl = vnt_controller_new(c"{\"input_method\":\"vni\"}".as_ptr(), VNT_MODE_PREEDIT);
            assert!(!ctrl.is_null());
            let mut effects = VntEffectList::EMPTY;
            assert!(vnt_controller_apply_config_json(
                ctrl,
                c"{\"tone_style\":\"old\"}".as_ptr(),
                &mut effects
            ));
            assert!(!vnt_controller_apply_config_json(ctrl, c"{bad".as_ptr(), &mut effects));

            vnt_controller_focus_changed(ctrl, c"alacritty".as_ptr());
            assert_eq!(vnt_controller_profile_output_mode(ctrl), 2); // Replace

            assert_eq!(vnt_char_to_utf16_offset(c"😀a".as_ptr(), 1), 2);
            assert_eq!(vnt_char_to_byte_offset(c"😀a".as_ptr(), 1), 4);
            assert_eq!(vnt_utf16_to_byte_offset(c"😀a".as_ptr(), 2), 4);
            assert_eq!(vnt_utf16_to_byte_offset(c"😀a".as_ptr(), 1), u32::MAX);

            let list = CStr::from_ptr(vnt_charset_list()).to_str().unwrap();
            assert!(list.contains("tcvn3\t"));

            let viqr = CStr::from_ptr(vnt_charset_encode(ctrl, c"viqr".as_ptr(), c"Việt".as_ptr()))
                .to_str()
                .unwrap()
                .to_string();
            assert_eq!(viqr, "Vie^.t");
            let back = CStr::from_ptr(vnt_charset_decode(ctrl, c"viqr".as_ptr(), c"Vie^.t".as_ptr()))
                .to_str()
                .unwrap();
            assert_eq!(back, "Việt");

            let mut out_len = 0usize;
            let bytes_ptr =
                vnt_charset_encode_bytes(ctrl, c"tcvn3".as_ptr(), c"Tiếng Việt".as_ptr(), &mut out_len);
            assert!(!bytes_ptr.is_null() && out_len > 0);
            let decoded =
                CStr::from_ptr(vnt_charset_decode_bytes(ctrl, c"tcvn3".as_ptr(), bytes_ptr, out_len))
                    .to_str()
                    .unwrap();
            assert_eq!(decoded, "Tiếng Việt");

            vnt_controller_free(ctrl);
        }
    }
}
