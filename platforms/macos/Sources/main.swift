import Cocoa
import InputMethodKit

/// Điểm vào khởi tạo dịch vụ `IMKServer` và bộ theo dõi tệp cấu hình `kqueue` trên macOS.
@main
enum VNTypeMacMain {
    static var server: IMKServer?

    static func main() {
        let bundleID = Bundle.main.bundleIdentifier ?? "dev.hephaestus.inputmethod.VNType"
        let connectionName = (Bundle.main.infoDictionary?["InputMethodConnectionName"] as? String)
            ?? "dev.hephaestus.inputmethod.VNType_Connection"
        server = IMKServer(name: connectionName, bundleIdentifier: bundleID)
        if let configURL = VNTypeInputController.defaultConfigURL() {
            ConfigWatcher.shared.startWatching(configURL: configURL)
        }
        NSApplication.shared.run()
    }
}
