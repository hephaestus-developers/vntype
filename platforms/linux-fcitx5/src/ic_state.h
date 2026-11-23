/**
 * @file ic_state.h
 * @brief Quản lý trạng thái bộ gõ VNType cho từng ngữ cảnh nhập liệu (`fcitx::InputContext`).
 */

#pragma once

#include <cstdint>
#include <filesystem>
#include <memory>
#include <string>

#include <fcitx-utils/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>

#include "vntype.h"

namespace fcitx {

/**
 * @brief Lớp lưu giữ con trỏ `VntController*` và bộ hẹn giờ cho một cửa sổ nhập liệu.
 *
 * Toàn bộ logic bảo vệ văn bản xung quanh, hàng đợi giao dịch và xử lý Fail-Safe đều nằm
 * trong `VntController` của lõi Rust; lớp này chỉ duyệt `VntEffectList` và gọi API Fcitx5.
 */
class VntypeIcState final : public InputContextProperty {
public:
    VntypeIcState(EventLoop &eventLoop, InputContext *ic, const std::string &initialConfigJson);
    ~VntypeIcState() override;

    void activate(const std::string &program);
    void deactivate(InputContextEvent &event);
    void reset();
    void keyEvent(KeyEvent &event);
    void toggleVietnamese();
    void reloadConfig(const std::string &configJson);

    bool isEnabled() const;

    /** Thực thi tuần tự danh sách hiệu ứng nguyên tử trả về từ C ABI của Rust. */
    void executeEffects(const VntEffectList &list, KeyEvent *currentEvent);

private:
    static std::string translateKeyName(KeySym sym, KeyStates states);
    void syncSurrounding();
    void syncFieldKind();
    void syncOutputMode();
    void scheduleTimer(uint32_t waitMs);
    void clearPreedit();
    void maybeReloadConfig();

    EventLoop &eventLoop_;
    InputContext *ic_ = nullptr;
    VntController *controller_ = nullptr;
    std::unique_ptr<EventSourceTime> timer_;
    std::filesystem::path configPath_;
    std::filesystem::file_time_type configMtime_{};
    bool hasConfigMtime_ = false;
    uint32_t fieldKind_ = VNT_FIELD_TEXT;
    uint32_t mode_ = VNT_MODE_PREEDIT;
    bool usingClientPreedit_ = false;
    bool hasPreedit_ = false;
};

} // namespace fcitx
