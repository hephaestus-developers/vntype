//! Integration tests kiểm tra giao diện C ABI của `vntype-ffi`.

use std::ffi::CStr;
use std::ptr;

use vntype_ffi::{
    VNT_EFFECT_COMMIT_TEXT, VNT_EFFECT_WAIT_MS, VNT_MODE_PREEDIT, vnt_controller_display_text,
    vnt_controller_free, vnt_controller_is_busy, vnt_controller_new, vnt_controller_on_timer_tick,
    vnt_controller_process_char, vnt_controller_set_async_replace, vnt_controller_set_surrounding_utf16,
};

#[test]
fn c_abi_async_replace_and_process_char_flow() {
    unsafe {
        let ctrl = vnt_controller_new(ptr::null(), VNT_MODE_PREEDIT);
        assert!(!ctrl.is_null());

        vnt_controller_set_async_replace(ctrl, true);
        vnt_controller_set_surrounding_utf16(ctrl, c"Chào".as_ptr(), 4, 4);

        let list = vnt_controller_process_char(ctrl, 's' as u32, false, false, false, 31);
        assert!(vnt_controller_is_busy(ctrl));
        let effs = std::slice::from_raw_parts(list.effects, list.count as usize);
        assert!(effs.iter().any(|e| e.kind == VNT_EFFECT_WAIT_MS));

        let tick = vnt_controller_on_timer_tick(ctrl);
        assert!(!vnt_controller_is_busy(ctrl));
        let tick_effs = std::slice::from_raw_parts(tick.effects, tick.count as usize);
        assert!(tick_effs.iter().any(|e| {
            e.kind == VNT_EFFECT_COMMIT_TEXT && CStr::from_ptr(e.text).to_str().unwrap() == "áo"
        }));
        assert_eq!(CStr::from_ptr(vnt_controller_display_text(ctrl)).to_str().unwrap(), "Cháo");

        vnt_controller_free(ctrl);
    }
}
