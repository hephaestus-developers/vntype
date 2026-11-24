import Cocoa
import SwiftUI
import ApplicationServices

/// Trình quản lý cửa sổ cấu hình bản địa (Singleton) dùng `NSHostingController`.
final class PreferencesWindowController {
    static let shared = PreferencesWindowController()

    private var window: NSWindow?

    private init() {}

    func show() {
        if let existing = window {
            existing.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.1.0"
        let hosting = NSHostingController(rootView: PreferencesView())
        let win = NSWindow(contentViewController: hosting)
        win.title = "VNType \(version)"
        win.styleMask = [.titled, .closable, .miniaturizable]
        win.isReleasedWhenClosed = false
        win.center()
        win.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        window = win
    }
}

/// Giao diện cấu hình bản địa viết bằng SwiftUI cho ứng dụng VNType trên macOS.
struct PreferencesView: View {
    @State private var charset: String = "unicode"
    @State private var inputMethod: String = "telex"
    @State private var toneStyle: String = "new"

    @State private var defaultVietnamese: Bool = true
    @State private var spellCheck: Bool = true
    @State private var wAsU: Bool = false
    @State private var bracketKeys: Bool = false
    @State private var quickTelex: Bool = false
    @State private var allowCodaK: Bool = false
    @State private var editPreviousWord: Bool = true
    @State private var autoCapitalize: Bool = false
    @State private var smartCaps: Bool = false

    @State private var macroEnabled: Bool = true
    @State private var macroUndo: Bool = true
    @State private var macroDigits: Bool = false
    @State private var macroCount: Int = 0

    @State private var savedSnapshot: [String: Any] = [:]
    @State private var errorMessage: String = ""
    @State private var showInfoAlert: Bool = false
    @State private var accessibilityTrusted: Bool = AXIsProcessTrusted()

    private let charsets = VntBridge.charsetList()
    private let caps = VntBridge.settingsCapabilities()

    private var configPath: String? {
        VNTypeInputController.defaultConfigURL()?.path
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupBox("Điều khiển") {
                Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
                    GridRow {
                        Text("Bảng mã:")
                        Picker("", selection: $charset) {
                            ForEach(charsets, id: \.key) { item in
                                Text(item.name).tag(item.key)
                            }
                        }
                        .labelsHidden()
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    GridRow {
                        Text("Kiểu gõ:")
                        Picker("", selection: $inputMethod) {
                            Text("Telex").tag("telex")
                            Text("Simple Telex").tag("simple_telex")
                            Text("VNI").tag("vni")
                            Text("VIQR").tag("viqr")
                        }
                        .labelsHidden()
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    GridRow {
                        Text("Đặt dấu:")
                        Picker("", selection: $toneStyle) {
                            Text("Mới (hoà, thuý)").tag("new")
                            Text("Cũ (hòa, thúy)").tag("old")
                        }
                        .pickerStyle(.radioGroup)
                        .horizontalRadioGroupLayout()
                        .labelsHidden()
                    }
                }
                .padding(8)
            }

            GroupBox("Tùy chọn khác") {
                HStack(alignment: .top, spacing: 20) {
                    VStack(alignment: .leading, spacing: 6) {
                        Toggle("Bật tiếng Việt mặc định", isOn: $defaultVietnamese)
                        Toggle("Bật kiểm tra chính tả", isOn: $spellCheck)
                        Toggle("Phím w đứng riêng thành ư (Telex)", isOn: $wAsU)
                        Toggle("Phím [ / ] thành ơ / ư (Telex)", isOn: $bracketKeys)
                        Toggle("Quick Telex (cc=ch, gg=gi...)", isOn: $quickTelex)
                    }
                    VStack(alignment: .leading, spacing: 6) {
                        Toggle("Âm cuối k với sắc/nặng (Đắk Lắk)", isOn: $allowCodaK)
                        Toggle("Sửa từ trước bằng Backspace", isOn: $editPreviousWord)
                        Toggle("Tự viết hoa đầu câu", isOn: $autoCapitalize)
                        Toggle("Giữ chữ hoa 2 chữ cái đầu", isOn: $smartCaps)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
            }

            GroupBox("Tùy chọn gõ tắt") {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(alignment: .top, spacing: 20) {
                        VStack(alignment: .leading, spacing: 6) {
                            Toggle("Cho phép gõ tắt", isOn: $macroEnabled)
                            Toggle("Cho phép chữ số trong gõ tắt", isOn: $macroDigits)
                        }
                        VStack(alignment: .leading, spacing: 6) {
                            Toggle("Backspace hoàn tác gõ tắt", isOn: $macroUndo)
                        }
                    }
                    if caps?.supports_macro_file_editor ?? true {
                        HStack(spacing: 8) {
                            Button("Bảng gõ tắt (macros.txt)...") {
                                openMacrosFile()
                            }
                            Text("(\(macroCount) quy tắc)")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
            }

            if caps?.supports_accessibility_check ?? true {
                GroupBox("Quyền hệ thống (macOS)") {
                    HStack {
                        Image(systemName: accessibilityTrusted ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                            .foregroundColor(accessibilityTrusted ? .green : .orange)
                        Text(accessibilityTrusted
                             ? "Đã cấp quyền Accessibility (Trợ năng)"
                             : "Chưa cấp quyền Accessibility — cần thiết cho chế độ gõ trực tiếp")
                            .font(.callout)
                        Spacer()
                        if !accessibilityTrusted {
                            Button("Mở Cài đặt...") {
                                if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
                                    NSWorkspace.shared.open(url)
                                }
                            }
                        }
                    }
                    .padding(8)
                }
            }

            if !errorMessage.isEmpty {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundColor(.red)
            }

            HStack(spacing: 8) {
                Button("Thông tin") {
                    showInfoAlert = true
                }
                Spacer()
                Button("Mặc định") {
                    resetToDefaults()
                }
                Button("Hoàn tác") {
                    applyDictionary(savedSnapshot)
                }
                .disabled(!isDirty)
                Button("Lưu & Đóng") {
                    saveAndClose()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(16)
        .frame(width: 560)
        .onAppear {
            loadConfig()
            accessibilityTrusted = AXIsProcessTrusted()
        }
        .alert("Thông tin VNType", isPresented: $showInfoAlert) {
            Button("OK", role: .cancel) {}
        } message: {
            let cfg = configPath ?? "~/Library/Application Support/VNType/config.json"
            let mac = VNTypeInputController.defaultMacrosURL()?.path ?? "~/Library/Application Support/VNType/macros.txt"
            Text("Cấu hình: \(cfg)\nGõ tắt: \(mac)\n\nChuyển Anh/Việt trên macOS qua phím tắt Input Source hệ thống.")
        }
    }

    private var isDirty: Bool {
        currentPayload() as NSDictionary != savedSnapshot as NSDictionary
    }

    private func currentPayload() -> [String: Any] {
        [
            "charset": charset,
            "unicode_form": charset == "unicode_nfd" ? "NFD" : "NFC",
            "input_method": inputMethod,
            "tone_style": toneStyle,
            "default_vietnamese": defaultVietnamese,
            "spell_check": spellCheck,
            "w_as_u": wAsU,
            "bracket_keys": bracketKeys,
            "quick_telex": quickTelex,
            "allow_coda_k": allowCodaK,
            "edit_previous_word": editPreviousWord,
            "auto_capitalize": autoCapitalize,
            "smart_caps": smartCaps,
            "macro_enabled": macroEnabled,
            "macro_undo": macroUndo,
            "macro_digits": macroDigits,
        ]
    }

    private func applyDictionary(_ dict: [String: Any]) {
        if let v = dict["charset"] as? String { charset = v }
        if let v = dict["input_method"] as? String { inputMethod = v }
        if let v = dict["tone_style"] as? String { toneStyle = v }
        if let v = dict["default_vietnamese"] as? Bool { defaultVietnamese = v }
        if let v = dict["spell_check"] as? Bool { spellCheck = v }
        if let v = dict["w_as_u"] as? Bool { wAsU = v }
        if let v = dict["bracket_keys"] as? Bool { bracketKeys = v }
        if let v = dict["quick_telex"] as? Bool { quickTelex = v }
        if let v = dict["allow_coda_k"] as? Bool { allowCodaK = v }
        if let v = dict["edit_previous_word"] as? Bool { editPreviousWord = v }
        if let v = dict["auto_capitalize"] as? Bool { autoCapitalize = v }
        if let v = dict["smart_caps"] as? Bool { smartCaps = v }
        if let v = dict["macro_enabled"] as? Bool { macroEnabled = v }
        if let v = dict["macro_undo"] as? Bool { macroUndo = v }
        if let v = dict["macro_digits"] as? Bool { macroDigits = v }
    }

    private func resetToDefaults() {
        charset = "unicode"
        inputMethod = "telex"
        toneStyle = "new"
        defaultVietnamese = true
        spellCheck = true
        wAsU = false
        bracketKeys = false
        quickTelex = false
        allowCodaK = false
        editPreviousWord = true
        autoCapitalize = false
        smartCaps = false
        macroEnabled = true
        macroUndo = true
        macroDigits = false
    }

    private func loadConfig() {
        errorMessage = ""
        macroCount = VntBridge.macroEntryCount(configPath: configPath)
        guard let json = VntBridge.loadSettingsJSON(configPath: configPath),
              let data = json.data(using: .utf8),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        applyDictionary(dict)
        savedSnapshot = currentPayload()
    }

    private func openMacrosFile() {
        guard VntBridge.ensureMacrosFile(configPath: configPath),
              let url = VNTypeInputController.defaultMacrosURL() else { return }
        NSWorkspace.shared.open(url)
        macroCount = VntBridge.macroEntryCount(configPath: configPath)
    }

    private func saveAndClose() {
        errorMessage = ""
        let payload = currentPayload()
        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys]),
              let json = String(data: data, encoding: .utf8) else {
            errorMessage = "Lỗi tạo JSON cấu hình"
            return
        }
        if VntBridge.saveSettingsJSON(json, configPath: configPath) {
            savedSnapshot = payload
            NSApp.keyWindow?.close()
        } else {
            errorMessage = "Không thể lưu cấu hình (kiểm tra lại giá trị hợp lệ)"
        }
    }
}
