/**
 * @file ic_state.cpp
 * @brief Cài đặt bộ thực thi hiệu ứng nguyên tử `VntEffectList` trên Fcitx5.
 */

#include "ic_state.h"

#include <algorithm>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/log.h>
#include <fcitx-utils/textformatflags.h>
#include <fcitx/inputpanel.h>
#include <fcitx/text.h>
#include <fcitx/userinterface.h>

namespace fcitx {

namespace {

std::filesystem::path resolveConfigPath() {
    if (const char *custom = std::getenv("VNTYPE_CONFIG"); custom && *custom) {
        return custom;
    }
    if (const char *dir = std::getenv("VNTYPE_CONFIG_DIR"); dir && *dir) {
        return std::filesystem::path(dir) / "config.json";
    }
    if (const char *xdg = std::getenv("XDG_CONFIG_HOME"); xdg && *xdg) {
        return std::filesystem::path(xdg) / "vntype" / "config.json";
    }
    if (const char *home = std::getenv("HOME"); home && *home) {
        return std::filesystem::path(home) / ".config" / "vntype" / "config.json";
    }
    return "config.json";
}

std::string readFileIfExists(const std::filesystem::path &path) {
    std::ifstream in(path);
    if (!in.is_open()) {
        return {};
    }
    std::ostringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

} // namespace

VntypeIcState::VntypeIcState(EventLoop &eventLoop, InputContext *ic,
                             const std::string &initialConfigJson)
    : eventLoop_(eventLoop), ic_(ic), configPath_(resolveConfigPath()) {
    std::string cfgJson = initialConfigJson;
    if (cfgJson.empty()) {
        cfgJson = readFileIfExists(configPath_);
        std::error_code ec;
        auto mtime = std::filesystem::last_write_time(configPath_, ec);
        if (!ec) {
            configMtime_ = mtime;
            hasConfigMtime_ = true;
        }
    }
    controller_ = vnt_controller_new(cfgJson.empty() ? nullptr : cfgJson.c_str(), VNT_MODE_PREEDIT);
    if (!controller_) {
        controller_ = vnt_controller_new(nullptr, VNT_MODE_PREEDIT);
    }
    if (controller_) {
        vnt_controller_set_async_replace(controller_, true);
    }
}

VntypeIcState::~VntypeIcState() {
    timer_.reset();
    if (controller_) {
        vnt_controller_free(controller_);
        controller_ = nullptr;
    }
}

std::string VntypeIcState::translateKeyName(KeySym sym, KeyStates states) {
    switch (sym) {
    case FcitxKey_BackSpace: return "Backspace";
    case FcitxKey_Return:
    case FcitxKey_KP_Enter: return "Enter";
    case FcitxKey_Tab: return "Tab";
    case FcitxKey_Escape: return "Escape";
    case FcitxKey_Left: return "Left";
    case FcitxKey_Right: return "Right";
    case FcitxKey_Up: return "Up";
    case FcitxKey_Down: return "Down";
    case FcitxKey_Home: return "Home";
    case FcitxKey_End: return "End";
    case FcitxKey_Delete: return "Delete";
    case FcitxKey_Page_Up: return "PageUp";
    case FcitxKey_Page_Down: return "PageDown";
    default: break;
    }
    std::string text = Key::keySymToUTF8(sym);
    if (text.empty()) {
        return {};
    }
    if (states.test(KeyState::Shift) && text.size() == 1 && text[0] >= 'a' && text[0] <= 'z') {
        text[0] = static_cast<char>(text[0] - 'a' + 'A');
    }
    return text;
}

bool VntypeIcState::isEnabled() const {
    return controller_ && vnt_controller_is_enabled(controller_);
}

void VntypeIcState::activate(const std::string &program) {
    if (!controller_) {
        return;
    }
    VntEffectList list = vnt_controller_focus_changed(
        controller_, program.empty() ? nullptr : program.c_str());
    executeEffects(list, nullptr);
    syncFieldKind();
    syncOutputMode();
}

void VntypeIcState::deactivate(InputContextEvent &event) {
    if (!controller_) {
        return;
    }
    timer_.reset();
    const bool focusOut = event.type() == EventType::InputContextFocusOut;
    if (focusOut && usingClientPreedit_ && hasPreedit_) {
        // Lõi Fcitx5 đã tự động chốt clientPreedit khi mất tiêu điểm; chỉ reset để tránh nhân đôi từ
        vnt_controller_reset(controller_);
        clearPreedit();
        return;
    }
    VntEffectList list = vnt_controller_flush(controller_);
    executeEffects(list, nullptr);
    vnt_controller_reset(controller_);
    clearPreedit();
}

void VntypeIcState::reset() {
    if (!controller_) {
        return;
    }
    timer_.reset();
    VntEffectList list = vnt_controller_flush(controller_);
    executeEffects(list, nullptr);
    vnt_controller_reset(controller_);
    clearPreedit();
}

void VntypeIcState::toggleVietnamese() {
    if (!controller_) {
        return;
    }
    VntEffectList list = vnt_controller_toggle_vietnamese(controller_);
    executeEffects(list, nullptr);
}

void VntypeIcState::reloadConfig(const std::string &configJson) {
    if (!controller_ || vnt_controller_is_busy(controller_)) {
        return;
    }
    VntEffectList list = {};
    if (vnt_controller_apply_config_json(
            controller_, configJson.empty() ? "{}" : configJson.c_str(), &list)) {
        executeEffects(list, nullptr);
    }
}

void VntypeIcState::syncSurrounding() {
    if (!controller_ || !ic_) {
        return;
    }
    if (!ic_->capabilityFlags().test(CapabilityFlag::SurroundingText) ||
        !ic_->surroundingText().isValid()) {
        vnt_controller_set_surrounding_utf16(controller_, nullptr, 0, 0);
        return;
    }
    const auto &sur = ic_->surroundingText();
    const std::string &text = sur.text();
    const uint32_t c = vnt_char_to_utf16_offset(text.c_str(), sur.cursor());
    const uint32_t a = vnt_char_to_utf16_offset(text.c_str(), sur.anchor());
    if (c == UINT32_MAX || a == UINT32_MAX) {
        vnt_controller_set_surrounding_utf16(controller_, nullptr, 0, 0);
        return;
    }
    vnt_controller_set_surrounding_utf16(controller_, text.c_str(), c, a);
}

void VntypeIcState::syncFieldKind() {
    if (!controller_ || !ic_) {
        return;
    }
    const auto flags = ic_->capabilityFlags();
    const bool isUrl = flags.test(CapabilityFlag::Url);
    uint32_t kind = VNT_FIELD_TEXT;
    if (flags.test(CapabilityFlag::Disable) || flags.test(CapabilityFlag::Password) ||
        (flags.test(CapabilityFlag::Sensitive) && !isUrl)) {
        kind = VNT_FIELD_PASSWORD;
    } else if (isUrl) {
        kind = VNT_FIELD_URL;
    } else if (flags.test(CapabilityFlag::Email)) {
        kind = VNT_FIELD_EMAIL;
    } else if (flags.test(CapabilityFlag::Number) || flags.test(CapabilityFlag::Digit) ||
               flags.test(CapabilityFlag::Dialable)) {
        kind = VNT_FIELD_NUMBER;
    } else if (flags.test(CapabilityFlag::Terminal)) {
        kind = VNT_FIELD_TERMINAL;
    }
    if (kind != fieldKind_) {
        fieldKind_ = kind;
        VntEffectList list = vnt_controller_set_field_kind(controller_, kind);
        executeEffects(list, nullptr);
    }
}

void VntypeIcState::syncOutputMode() {
    if (!controller_ || !ic_) {
        return;
    }
    const uint32_t wanted = vnt_controller_profile_output_mode(controller_);
    const bool hasSur = ic_->capabilityFlags().test(CapabilityFlag::SurroundingText) &&
                        ic_->surroundingText().isValid();
    const uint32_t target = (wanted == 2 && hasSur) ? VNT_MODE_REPLACE : VNT_MODE_PREEDIT;
    if (target != mode_) {
        mode_ = target;
        VntEffectList list = vnt_controller_set_output_mode(controller_, target);
        executeEffects(list, nullptr);
    }
}

void VntypeIcState::keyEvent(KeyEvent &event) {
    if (!controller_ || event.isRelease()) {
        return;
    }
    const Key raw = event.rawKey();
    const KeyStates states = raw.states();
    const bool ctrl = states.test(KeyState::Ctrl);
    const bool alt = states.test(KeyState::Alt);
    const bool meta = states.test(KeyState::Super) || states.test(KeyState::Meta);
    const std::string key = translateKeyName(raw.sym(), states);
    if (key.empty()) {
        return;
    }

    if (!vnt_controller_is_busy(controller_)) {
        if (!hasPreedit_) {
            maybeReloadConfig();
        }
        syncFieldKind();
        syncOutputMode();
        syncSurrounding();
    }

    VntEffectList list = vnt_controller_process_key(
        controller_, key.c_str(), ctrl, alt, meta, static_cast<uint32_t>(raw.sym()));
    executeEffects(list, &event);
}

void VntypeIcState::executeEffects(const VntEffectList &list, KeyEvent *currentEvent) {
    if (!ic_) {
        return;
    }
    bool forwardOriginal = false;
    for (uint32_t i = 0; i < list.count; ++i) {
        const VntEffect &eff = list.effects[i];
        const std::string text = eff.text ? eff.text : "";
        switch (eff.kind) {
        case VNT_EFFECT_SET_PREEDIT: {
            auto &panel = ic_->inputPanel();
            panel.reset();
            hasPreedit_ = !text.empty();
            if (!text.empty()) {
                Text preedit(text, TextFormatFlag::Underline);
                preedit.setCursor(static_cast<int>(std::min<size_t>(eff.cursor_bytes, text.size())));
                usingClientPreedit_ = ic_->capabilityFlags().test(CapabilityFlag::Preedit);
                if (usingClientPreedit_) {
                    panel.setClientPreedit(preedit);
                } else {
                    panel.setPreedit(preedit);
                }
            }
            ic_->updatePreedit();
            ic_->updateUserInterface(UserInterfaceComponent::InputPanel);
            break;
        }
        case VNT_EFFECT_CLEAR_PREEDIT:
            clearPreedit();
            break;
        case VNT_EFFECT_COMMIT_TEXT:
            if (!text.empty()) {
                ic_->commitString(text);
            }
            break;
        case VNT_EFFECT_REPLACE_TEXT:
            if (eff.delete_chars > 0) {
                ic_->deleteSurroundingText(-static_cast<int>(eff.delete_chars), eff.delete_chars);
            }
            if (!text.empty()) {
                ic_->commitString(text);
            }
            break;
        case VNT_EFFECT_WAIT_MS:
            scheduleTimer(eff.wait_ms);
            break;
        case VNT_EFFECT_FORWARD_KEY:
            if (currentEvent) {
                forwardOriginal = true;
            } else if (eff.raw_keycode != 0) {
                Key forwarded(static_cast<KeySym>(eff.raw_keycode));
                ic_->forwardKey(forwarded, false);
                ic_->forwardKey(forwarded, true);
            }
            break;
        default:
            break;
        }
    }
    if (currentEvent && !forwardOriginal) {
        currentEvent->filterAndAccept();
    }
}

void VntypeIcState::scheduleTimer(uint32_t waitMs) {
    timer_.reset();
    const uint64_t deadline = now(CLOCK_MONOTONIC) + static_cast<uint64_t>(waitMs) * 1000;
    timer_ = eventLoop_.addTimeEvent(
        CLOCK_MONOTONIC, deadline, 1000, [this](EventSourceTime *, uint64_t) {
            auto done = std::move(timer_);
            (void)done;
            if (controller_) {
                VntEffectList list = vnt_controller_on_timer_tick(controller_);
                executeEffects(list, nullptr);
            }
            return false;
        });
    if (timer_) {
        timer_->setOneShot();
    }
}

void VntypeIcState::clearPreedit() {
    if (!ic_) {
        return;
    }
    ic_->inputPanel().reset();
    ic_->updatePreedit();
    ic_->updateUserInterface(UserInterfaceComponent::InputPanel);
    usingClientPreedit_ = false;
    hasPreedit_ = false;
}

void VntypeIcState::maybeReloadConfig() {
    std::error_code ec;
    auto mtime = std::filesystem::last_write_time(configPath_, ec);
    if (ec) {
        return;
    }
    if (!hasConfigMtime_ || mtime != configMtime_) {
        configMtime_ = mtime;
        hasConfigMtime_ = true;
        std::string json = readFileIfExists(configPath_);
        if (!json.empty()) {
            reloadConfig(json);
        }
    }
}

} // namespace fcitx
