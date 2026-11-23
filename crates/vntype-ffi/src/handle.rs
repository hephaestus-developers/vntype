//! Quản lý đối tượng mờ [`VntController`] và vùng đệm bộ nhớ C (`Arena Buffers`) không rò rỉ.

use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use vntype_adapter::{AdapterController, Effect};
use vntype_engine::FieldKind;

use crate::c_types::{
    VNT_EFFECT_CLEAR_PREEDIT, VNT_EFFECT_COMMIT_TEXT, VNT_EFFECT_FORWARD_KEY, VNT_EFFECT_REPLACE_TEXT,
    VNT_EFFECT_SET_PREEDIT, VNT_EFFECT_WAIT_MS, VNT_FIELD_EMAIL, VNT_FIELD_NUMBER, VNT_FIELD_PASSWORD,
    VNT_FIELD_TERMINAL, VNT_FIELD_URL, VntEffect, VntEffectList,
};

/// Đối tượng điều khiển phiên gõ phía sau con trỏ mờ C ABI, giữ sẵn các vùng đệm chuỗi.
pub struct VntController {
    /// Bộ điều khiển lõi Rust.
    pub inner: AdapterController,
    /// Vùng đệm lưu giữ các chuỗi `CString` cho danh sách hiệu ứng gần nhất.
    pub effect_strings: Vec<CString>,
    /// Vùng đệm lưu giữ mảng `VntEffect` gần nhất.
    pub effect_items: Vec<VntEffect>,
    /// Vùng đệm lưu chuỗi `vnt_controller_display_text`.
    pub display_buf: CString,
    /// Vùng đệm lưu chuỗi JSON cấu hình hoặc hồ sơ ứng dụng.
    pub json_buf: CString,
    /// Vùng đệm lưu chuỗi kết quả chuyển đổi bảng mã.
    pub charset_buf: CString,
    /// Vùng đệm lưu mảng byte kết quả chuyển đổi bảng mã một byte.
    pub bytes_buf: Vec<u8>,
}

impl VntController {
    /// Khởi tạo [`VntController`] từ một [`AdapterController`].
    pub fn new(inner: AdapterController) -> Self {
        VntController {
            inner,
            effect_strings: Vec::with_capacity(4),
            effect_items: Vec::with_capacity(4),
            display_buf: CString::default(),
            json_buf: CString::default(),
            charset_buf: CString::default(),
            bytes_buf: Vec::new(),
        }
    }

    /// Chuyển danh sách [`Effect`] của Rust sang [`VntEffectList`] của C, giữ sống toàn bộ con trỏ chuỗi.
    pub fn pack_effects(&mut self, effects: Vec<Effect>) -> VntEffectList {
        self.effect_strings.clear();
        self.effect_items.clear();
        // Đảm bảo vector chuỗi không tái cấp phát làm đổi địa chỉ con trỏ trong lúc lặp
        self.effect_strings.reserve(effects.len());
        self.effect_items.reserve(effects.len());

        for eff in effects {
            match eff {
                Effect::SetPreedit { text, cursor_utf16, cursor_bytes } => {
                    let cstr = CString::new(text.replace('\0', "")).unwrap_or_default();
                    let ptr = cstr.as_ptr();
                    self.effect_strings.push(cstr);
                    self.effect_items.push(VntEffect {
                        kind: VNT_EFFECT_SET_PREEDIT,
                        cursor_utf16,
                        cursor_bytes,
                        text: ptr,
                        ..VntEffect::default()
                    });
                }
                Effect::ClearPreedit => {
                    self.effect_items
                        .push(VntEffect { kind: VNT_EFFECT_CLEAR_PREEDIT, ..VntEffect::default() });
                }
                Effect::CommitText { text } => {
                    let cstr = CString::new(text.replace('\0', "")).unwrap_or_default();
                    let ptr = cstr.as_ptr();
                    self.effect_strings.push(cstr);
                    self.effect_items.push(VntEffect {
                        kind: VNT_EFFECT_COMMIT_TEXT,
                        text: ptr,
                        ..VntEffect::default()
                    });
                }
                Effect::ReplaceText { delete_utf16, delete_chars, delete_bytes, insert } => {
                    let cstr = CString::new(insert.replace('\0', "")).unwrap_or_default();
                    let ptr = cstr.as_ptr();
                    self.effect_strings.push(cstr);
                    self.effect_items.push(VntEffect {
                        kind: VNT_EFFECT_REPLACE_TEXT,
                        delete_utf16,
                        delete_chars,
                        delete_bytes,
                        text: ptr,
                        ..VntEffect::default()
                    });
                }
                Effect::WaitMs { ms } => {
                    self.effect_items.push(VntEffect {
                        kind: VNT_EFFECT_WAIT_MS,
                        wait_ms: ms,
                        ..VntEffect::default()
                    });
                }
                Effect::ForwardKey { raw_keycode } => {
                    self.effect_items.push(VntEffect {
                        kind: VNT_EFFECT_FORWARD_KEY,
                        raw_keycode,
                        ..VntEffect::default()
                    });
                }
            }
        }

        if self.effect_items.is_empty() {
            VntEffectList::EMPTY
        } else {
            VntEffectList { effects: self.effect_items.as_ptr(), count: self.effect_items.len() as u32 }
        }
    }
}

/// Đọc con trỏ chuỗi C có thể NULL sang `Option<String>`.
pub unsafe fn opt_cstr(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// Ánh xạ mã số trường nhập liệu từ C ABI sang [`FieldKind`].
pub fn field_kind_from_u32(value: u32) -> FieldKind {
    match value {
        VNT_FIELD_PASSWORD => FieldKind::Password,
        VNT_FIELD_NUMBER => FieldKind::Number,
        VNT_FIELD_EMAIL => FieldKind::Email,
        VNT_FIELD_URL => FieldKind::Url,
        VNT_FIELD_TERMINAL => FieldKind::Terminal,
        _ => FieldKind::Text,
    }
}

/// Bọc một hàm không gắn với controller trong `catch_unwind`.
pub fn guard<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}

/// Bọc một thao tác trên con trỏ `VntController` trong `catch_unwind`; nếu xảy ra panic sẽ tự động `reset()`.
pub unsafe fn with_controller<T>(
    h: *mut VntController,
    fallback: T,
    f: impl FnOnce(&mut VntController) -> T,
) -> T {
    if h.is_null() {
        return fallback;
    }
    match catch_unwind(AssertUnwindSafe(|| f(unsafe { &mut *h }))) {
        Ok(v) => v,
        Err(_) => {
            let _ = catch_unwind(AssertUnwindSafe(|| unsafe { (*h).inner.reset() }));
            fallback
        }
    }
}
