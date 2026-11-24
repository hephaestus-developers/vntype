import Foundation

/// Loại hiệu ứng nguyên tử trong đường ống xử lý của VNType.
enum VntEffectKind: UInt32 {
    /// Đặt văn bản gạch chân tạm thời (`SetPreedit`).
    case setPreedit = 1
    /// Xoá vùng văn bản gạch chân (`ClearPreedit`).
    case clearPreedit = 2
    /// Chốt chuỗi văn bản chính thức (`CommitText`).
    case commitText = 3
    /// Xoá lùi một đoạn văn bản và chèn văn bản mới (`ReplaceText`).
    case replaceText = 4
    /// Chờ một khoảng thời gian tính bằng mili-giây (`WaitMs`).
    case waitMs = 5
    /// Chuyển tiếp sự kiện phím gốc về cho ứng dụng (`ForwardKey`).
    case forwardKey = 6
}

/// Một hiệu ứng nguyên tử đã được chuyển sang kiểu dữ liệu an toàn của Swift.
struct VntSwiftEffect: Equatable {
    /// Phân loại hiệu ứng.
    let kind: VntEffectKind
    /// Số đơn vị mã UTF-16 cần xoá lùi.
    let deleteUTF16: Int
    /// Vị trí con trỏ theo đơn vị mã UTF-16.
    let cursorUTF16: Int
    /// Thời gian chờ (mili-giây).
    let waitMs: Int
    /// Nội dung văn bản đi kèm.
    let text: String
}

/// Giao diện trừu tượng hoá đối tượng nhận văn bản trên macOS (`IMKTextInput`).
///
/// Cho phép thực thi kiểm thử đơn vị không cần giao diện đồ hoạ (`MacAdapterTests`).
protocol MacTextClient: AnyObject {
    /// Đặt văn bản gạch chân tạm thời (Marked Text).
    func setMarkedText(_ text: String, cursorUTF16: Int)
    /// Xoá vùng gạch chân tạm thời.
    func clearMarkedText()
    /// Chèn văn bản đã chốt tại vị trí con trỏ.
    func commitText(_ text: String)
    /// Thay thế `deleteBeforeUTF16` đơn vị mã ngay trước con trỏ bằng chuỗi `text`.
    func replaceText(_ text: String, deleteBeforeUTF16: Int)
}

/// Lớp bọc Swift an toàn quản lý vòng đời con trỏ `VntController*` của lõi Rust.
final class VntBridge {
    private var handle: OpaquePointer?

    /// Khởi tạo bộ điều khiển với chuỗi JSON cấu hình và chế độ xuất chữ (`0 = Preedit`, `1 = Replace`).
    init(configJSON: String? = nil, mode: UInt32 = 0) {
        if let json = configJSON {
            handle = json.withCString { vnt_controller_new($0, mode) }
        } else {
            handle = vnt_controller_new(nil, mode)
        }
        if handle == nil {
            handle = vnt_controller_new(nil, mode)
        }
    }

    deinit {
        if let h = handle {
            vnt_controller_free(h)
        }
    }

    /// Chuyển đổi `VntEffectList` từ C ABI sang mảng `[VntSwiftEffect]`.
    private func unpack(_ list: VntEffectList) -> [VntSwiftEffect] {
        guard list.count > 0, let base = list.effects else { return [] }
        var out: [VntSwiftEffect] = []
        out.reserveCapacity(Int(list.count))
        for i in 0..<Int(list.count) {
            let raw = base[i]
            guard let kind = VntEffectKind(rawValue: raw.kind) else { continue }
            let text = raw.text.map { String(cString: $0) } ?? ""
            out.append(VntSwiftEffect(
                kind: kind,
                deleteUTF16: Int(raw.delete_utf16),
                cursorUTF16: Int(raw.cursor_utf16),
                waitMs: Int(raw.wait_ms),
                text: text
            ))
        }
        return out
    }

    /// Xử lý một phím theo tên chuẩn và trả về mảng hiệu ứng nguyên tử.
    func processKey(_ key: String, ctrl: Bool = false, alt: Bool = false,
                    meta: Bool = false, rawKeycode: UInt32 = 0) -> [VntSwiftEffect] {
        guard let h = handle else { return [] }
        let list = key.withCString {
            vnt_controller_process_key(h, $0, ctrl, alt, meta, rawKeycode)
        }
        return unpack(list)
    }

    /// Cập nhật ảnh chụp nhanh văn bản xung quanh con trỏ với toạ độ UTF-16.
    func setSurrounding(text: String?, cursorUTF16: UInt32, anchorUTF16: UInt32) {
        guard let h = handle else { return }
        if let text {
            text.withCString { vnt_controller_set_surrounding_utf16(h, $0, cursorUTF16, anchorUTF16) }
        } else {
            vnt_controller_set_surrounding_utf16(h, nil, 0, 0)
        }
    }

    /// Thông báo đổi ứng dụng nhận tiêu điểm.
    func focusChanged(appID: String?) -> [VntSwiftEffect] {
        guard let h = handle else { return [] }
        if let appID {
            return unpack(appID.withCString { vnt_controller_focus_changed(h, $0) })
        }
        return unpack(vnt_controller_focus_changed(h, nil))
    }

    /// Chốt vùng văn bản đang soạn dở.
    func flush() -> [VntSwiftEffect] {
        guard let h = handle else { return [] }
        return unpack(vnt_controller_flush(h))
    }

    /// Đảo trạng thái bật/tắt tiếng Việt.
    func toggleVietnamese() -> [VntSwiftEffect] {
        guard let h = handle else { return [] }
        return unpack(vnt_controller_toggle_vietnamese(h))
    }

    /// Đặt lại trạng thái bộ điều khiển mà không phát sinh hiệu ứng.
    func reset() {
        if let h = handle {
            vnt_controller_reset(h)
        }
    }

    /// Áp dụng mảnh JSON cấu hình mới tại chỗ (không cần khởi tạo lại `VntController`, giữ nguyên từ điển).
    func applyConfigJSON(_ json: String) -> [VntSwiftEffect]? {
        guard let h = handle else { return nil }
        var rawList = VntEffectList(effects: nil, count: 0)
        let ok = json.withCString { vnt_controller_apply_config_json(h, $0, &rawList) }
        guard ok else { return nil }
        return unpack(rawList)
    }

    /// Đồng bộ chế độ xuất chữ (`Preedit` / `Replace`) theo gợi ý từ `AppProfile` hiện tại nếu có.
    func syncProfileOutputMode() -> [VntSwiftEffect] {
        guard let h = handle else { return [] }
        let wanted = vnt_controller_profile_output_mode(h)
        switch wanted {
        case 1:
            return unpack(vnt_controller_set_output_mode(h, 0))
        case 2:
            return unpack(vnt_controller_set_output_mode(h, 1))
        default:
            return []
        }
    }

    /// Trả về chuỗi hiển thị hiện tại mà engine đang quản lý.
    func displayText() -> String {
        guard let h = handle, let ptr = vnt_controller_display_text(h) else { return "" }
        return String(cString: ptr)
    }

    /// Thực thi danh sách hiệu ứng nguyên tử lên một `MacTextClient`.
    ///
    /// Trả về `true` nếu sự kiện phím đã được bộ gõ tiêu thụ, `false` nếu cần trả phím về cho macOS.
    @discardableResult
    static func execute(_ effects: [VntSwiftEffect], on client: MacTextClient) -> Bool {
        var forwardOriginal = false
        for eff in effects {
            switch eff.kind {
            case .setPreedit:
                client.setMarkedText(eff.text, cursorUTF16: eff.cursorUTF16)
            case .clearPreedit:
                client.clearMarkedText()
            case .commitText:
                if !eff.text.isEmpty {
                    client.commitText(eff.text)
                }
            case .replaceText:
                client.replaceText(eff.text, deleteBeforeUTF16: eff.deleteUTF16)
            case .waitMs:
                break
            case .forwardKey:
                forwardOriginal = true
            }
        }
        return !effects.isEmpty && !forwardOriginal
    }

    /// Khả năng giao diện tương ứng với hệ điều hành macOS từ lõi Rust.
    struct SettingsCapabilities: Decodable {
        let supports_autostart: Bool
        let supports_daemon_ipc: Bool
        let supports_input_framework_status: Bool
        let supports_accessibility_check: Bool
        let supports_macro_file_editor: Bool
        let supports_switch_key: Bool
        let supports_excluded_apps: Bool
    }

    /// Trả về `SettingsCapabilities` của macOS qua FFI.
    static func settingsCapabilities() -> SettingsCapabilities? {
        guard let ptr = vnt_settings_capabilities_json() else { return nil }
        let json = String(cString: ptr)
        guard let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(SettingsCapabilities.self, from: data)
    }

    /// Trả về danh sách `(key, displayName)` các bảng mã được hỗ trợ từ lõi Rust.
    static func charsetList() -> [(key: String, name: String)] {
        guard let ptr = vnt_charset_list() else { return [] }
        return String(cString: ptr)
            .split(separator: "\n")
            .compactMap { line in
                let parts = line.split(separator: "\t", maxSplits: 1)
                guard parts.count == 2 else { return nil }
                return (String(parts[0]), String(parts[1]))
            }
    }

    /// Đọc JSON cấu hình từ `configPath` (hoặc đường dẫn mặc định) qua `vntype-ffi`.
    static func loadSettingsJSON(configPath: String?) -> String? {
        let rawPtr: UnsafeMutablePointer<CChar>? = if let path = configPath {
            path.withCString { vnt_settings_load_json($0) }
        } else {
            vnt_settings_load_json(nil)
        }
        guard let ptr = rawPtr else { return nil }
        defer { vnt_free_string(ptr) }
        return String(cString: ptr)
    }

    /// Kiểm tra hợp lệ và lưu nguyên tử bản vá JSON xuống `configPath` qua `vntype-ffi`.
    static func saveSettingsJSON(_ json: String, configPath: String?) -> Bool {
        if let path = configPath {
            return path.withCString { p in
                json.withCString { j in vnt_settings_validate_and_save_json(p, j) }
            }
        }
        return json.withCString { vnt_settings_validate_and_save_json(nil, $0) }
    }

    /// Đảm bảo tệp `macros.txt` tồn tại trên đĩa.
    @discardableResult
    static func ensureMacrosFile(configPath: String?) -> Bool {
        if let path = configPath {
            return path.withCString { vnt_macros_ensure_file($0) }
        }
        return vnt_macros_ensure_file(nil)
    }

    /// Đếm số lượng quy tắc gõ tắt đang có trong `macros.txt`.
    static func macroEntryCount(configPath: String?) -> Int {
        let count: Int32 = if let path = configPath {
            path.withCString { vnt_macros_entry_count($0) }
        } else {
            vnt_macros_entry_count(nil)
        }
        return max(0, Int(count))
    }
}
