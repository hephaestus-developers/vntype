import Carbon

/// Ánh xạ mã phím ảo Carbon (`kVK_*`) trên macOS sang tên phím chuẩn của VNType.
enum KeyMapper {
    static func name(for keyCode: UInt16) -> String? {
        switch Int(keyCode) {
        case kVK_Delete: return "Backspace"
        case kVK_ForwardDelete: return "Delete"
        case kVK_Return, kVK_ANSI_KeypadEnter: return "Enter"
        case kVK_Tab: return "Tab"
        case kVK_Escape: return "Escape"
        case kVK_LeftArrow: return "Left"
        case kVK_RightArrow: return "Right"
        case kVK_UpArrow: return "Up"
        case kVK_DownArrow: return "Down"
        case kVK_Home: return "Home"
        case kVK_End: return "End"
        case kVK_PageUp: return "PageUp"
        case kVK_PageDown: return "PageDown"
        default: return nil
        }
    }
}
