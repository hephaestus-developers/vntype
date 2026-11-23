/**
 * @file test_fcitx5_adapter.cpp
 * @brief Kiểm thử tích hợp CTest cho Fcitx5 addon sử dụng giao diện C ABI `vntype.h`.
 */

#include <cassert>
#include <cstring>

#include "vntype.h"

int main() {
    VntController *ctrl = vnt_controller_new(nullptr, VNT_MODE_PREEDIT);
    assert(ctrl != nullptr);

    const char *keys[] = {"t", "i", "e", "e", "n", "g", "s"};
    for (const char *k : keys) {
        VntEffectList list = vnt_controller_process_key(ctrl, k, false, false, false, 10);
        assert(list.count == 1);
        assert(list.effects[0].kind == VNT_EFFECT_SET_PREEDIT);
    }

    const char *disp = vnt_controller_display_text(ctrl);
    assert(disp != nullptr && std::strcmp(disp, "tiếng") == 0);

    VntEffectList commitList = vnt_controller_process_key(ctrl, " ", false, false, false, 57);
    assert(commitList.count == 2);
    assert(commitList.effects[0].kind == VNT_EFFECT_COMMIT_TEXT);
    assert(std::strcmp(commitList.effects[0].text, "tiếng ") == 0);
    assert(commitList.effects[1].kind == VNT_EFFECT_CLEAR_PREEDIT);

    // Kiểm tra hồ sơ ứng dụng và giao dịch Replace bất đồng bộ với timer
    vnt_controller_focus_changed(ctrl, "alacritty");
    assert(vnt_controller_profile_output_mode(ctrl) == 2);

    vnt_controller_focus_changed(ctrl, "chrome");
    vnt_controller_set_async_replace(ctrl, true);
    vnt_controller_set_surrounding_utf16(ctrl, "Chào", 4, 4);
    VntEffectList rep = vnt_controller_process_char(ctrl, static_cast<uint32_t>('s'), false, false, false, 31);
    assert(vnt_controller_is_busy(ctrl));
    assert(rep.count >= 2);
    assert(rep.effects[0].kind == VNT_EFFECT_REPLACE_TEXT);
    assert(rep.effects[1].kind == VNT_EFFECT_WAIT_MS);

    VntEffectList tick = vnt_controller_on_timer_tick(ctrl);
    assert(!vnt_controller_is_busy(ctrl));
    assert(tick.count >= 1);
    assert(tick.effects[0].kind == VNT_EFFECT_COMMIT_TEXT);
    assert(std::strcmp(tick.effects[0].text, "áo") == 0);

    vnt_controller_free(ctrl);
    return 0;
}
