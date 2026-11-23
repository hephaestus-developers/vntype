/**
 * @file engine.h
 * @brief Điểm kết nối addon VNType với `fcitx::InputMethodEngineV2`.
 */

#pragma once

#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>

#include "ic_state.h"

namespace fcitx {

/**
 * @brief Engine chính quản lý toàn bộ các `VntypeIcState` trong Fcitx5.
 */
class VntypeEngine final : public InputMethodEngineV2 {
public:
    explicit VntypeEngine(Instance *instance);

    void activate(const InputMethodEntry &entry, InputContextEvent &event) override;
    void deactivate(const InputMethodEntry &entry, InputContextEvent &event) override;
    void keyEvent(const InputMethodEntry &entry, KeyEvent &keyEvent) override;
    void reset(const InputMethodEntry &entry, InputContextEvent &event) override;

private:
    Instance *instance_;
    FactoryFor<VntypeIcState> factory_;
};

class VntypeEngineFactory final : public AddonFactory {
public:
    AddonInstance *create(AddonManager *manager) override {
        return new VntypeEngine(manager->instance());
    }
};

} // namespace fcitx
