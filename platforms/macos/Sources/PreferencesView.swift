import SwiftUI

/// Giao diện cấu hình bản địa viết bằng SwiftUI cho ứng dụng VNType trên macOS.
struct PreferencesView: View {
    @State private var inputMethod: String = "telex"
    @State private var toneStyle: String = "new"
    @State private var spellCheck: Bool = true
    @State private var autoCapitalize: Bool = false
    @State private var statusMessage: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Form {
                Picker("Kiểu gõ:", selection: $inputMethod) {
                    Text("Telex").tag("telex")
                    Text("Simple Telex").tag("simple_telex")
                    Text("VNI").tag("vni")
                    Text("VIQR").tag("viqr")
                }
                Picker("Quy tắc đặt dấu:", selection: $toneStyle) {
                    Text("Kiểu mới (hoà, thuý)").tag("new")
                    Text("Kiểu cũ (hòa, thúy)").tag("old")
                }
                Toggle("Kiểm tra chính tả tiếng Việt", isOn: $spellCheck)
                Toggle("Tự động viết hoa đầu câu", isOn: $autoCapitalize)
            }

            HStack {
                if !statusMessage.isEmpty {
                    Text(statusMessage)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                Spacer()
                Button("Lưu cấu hình") {
                    saveConfig()
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 420, height: 250)
        .onAppear(perform: loadConfig)
    }

    private func loadConfig() {
        guard let url = VNTypeInputController.defaultConfigURL(),
              let data = try? Data(contentsOf: url),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        if let method = dict["input_method"] as? String { inputMethod = method }
        if let tone = dict["tone_style"] as? String { toneStyle = tone }
        if let spell = dict["spell_check"] as? Bool { spellCheck = spell }
        if let cap = dict["auto_capitalize"] as? Bool { autoCapitalize = cap }
    }

    private func saveConfig() {
        guard let url = VNTypeInputController.defaultConfigURL() else { return }
        let dir = url.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        var dict: [String: Any] = [:]
        if let data = try? Data(contentsOf: url),
           let existing = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            dict = existing
        }
        dict["input_method"] = inputMethod
        dict["tone_style"] = toneStyle
        dict["spell_check"] = spellCheck
        dict["auto_capitalize"] = autoCapitalize

        guard let out = try? JSONSerialization.data(withJSONObject: dict, options: [.prettyPrinted, .sortedKeys]) else {
            statusMessage = "Lỗi tạo JSON"
            return
        }
        do {
            try out.write(to: url, options: .atomic)
            statusMessage = "Đã lưu"
        } catch {
            statusMessage = "Không thể lưu: \(error.localizedDescription)"
        }
    }
}
