/**
 * @file engine.cpp
 * @brief Cài đặt `VntypeEngine` và đăng ký macro `FCITX_ADDON_FACTORY`.
 */

#include "engine.h"

namespace fcitx {

VntypeEngine::VntypeEngine(Instance *instance)
    : instance_(instance),
      factory_([instance](InputContext &ic) {
          return new VntypeIcState(instance->eventLoop(), &ic, "");
      }) {
    instance_->inputContextManager().registerProperty("vntypeState", &factory_);
}

void VntypeEngine::activate(const InputMethodEntry &, InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (auto *state = ic->propertyFor(&factory_)) {
        state->activate(ic->program());
    }
}

void VntypeEngine::deactivate(const InputMethodEntry &, InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (auto *state = ic->propertyFor(&factory_)) {
        state->deactivate(event);
    }
}

void VntypeEngine::keyEvent(const InputMethodEntry &, KeyEvent &keyEvent) {
    auto *ic = keyEvent.inputContext();
    if (auto *state = ic->propertyFor(&factory_)) {
        state->keyEvent(keyEvent);
    }
}

void VntypeEngine::reset(const InputMethodEntry &, InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (auto *state = ic->propertyFor(&factory_)) {
        state->reset();
    }
}

} // namespace fcitx

FCITX_ADDON_FACTORY(fcitx::VntypeEngineFactory);
