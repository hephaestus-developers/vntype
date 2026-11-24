import Cocoa
import InputMethodKit
import Carbon

/// Bộ điều khiển `IMKInputController` chính trên macOS.
///
/// Toàn bộ quá trình lập kế hoạch, kiểm tra văn bản xung quanh và xử lý `FailSafe` (chốt từ trước khi
/// xoá preedit) được uỷ quyền hoàn toàn cho `VntBridge` (lõi Rust), đảm bảo không bao giờ làm mất chữ
/// đang gõ của người dùng.
@objc(VNTypeInputController)
final class VNTypeInputController: IMKInputController {
    private let bridge: VntBridge
    private var currentClient: AnyObject?

    override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        let initialJSON = Self.defaultConfigURL().flatMap { try? String(contentsOf: $0, encoding: .utf8) }
        self.bridge = VntBridge(configJSON: initialJSON, mode: 0)
        super.init(server: server, delegate: delegate, client: inputClient)
        currentClient = inputClient as AnyObject?
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(onConfigChanged(_:)),
            name: ConfigWatcher.configDidChangeNotification,
            object: nil
        )
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }

    @objc private func onConfigChanged(_ notification: Notification) {
        guard let url = Self.defaultConfigURL(),
              let json = try? String(contentsOf: url, encoding: .utf8),
              let effects = bridge.applyConfigJSON(json),
              let client = currentClient else { return }
        VntBridge.execute(effects, on: ImkTextClient(client))
    }

    override func activateServer(_ sender: Any!) {
        super.activateServer(sender)
        currentClient = sender as AnyObject?
        let appID = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        var effects = bridge.focusChanged(appID: appID)
        effects.append(contentsOf: bridge.syncProfileOutputMode())
        if let client = currentClient {
            VntBridge.execute(effects, on: ImkTextClient(client))
        }
    }

    override func deactivateServer(_ sender: Any!) {
        commitComposition(sender)
        super.deactivateServer(sender)
        currentClient = nil
    }

    override func commitComposition(_ sender: Any!) {
        let effects = bridge.flush()
        if let client = (sender as AnyObject?) ?? currentClient {
            VntBridge.execute(effects, on: ImkTextClient(client))
        }
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, event.type == .keyDown else { return false }
        guard let clientObj = (sender as AnyObject?) ?? currentClient else { return false }
        currentClient = clientObj
        let textClient = ImkTextClient(clientObj)

        // Khi macOS đang bật Secure Event Input (ô mật khẩu), huỷ vùng gạch chân và không can thiệp
        if IsSecureEventInputEnabled() {
            bridge.reset()
            textClient.clearMarkedText()
            return false
        }

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let ctrl = flags.contains(.control)
        let alt = flags.contains(.option)
        let meta = flags.contains(.command)

        if let snap = textClient.surroundingSnapshot() {
            bridge.setSurrounding(text: snap.text, cursorUTF16: snap.cursorUTF16, anchorUTF16: snap.anchorUTF16)
        } else {
            bridge.setSurrounding(text: nil, cursorUTF16: 0, anchorUTF16: 0)
        }

        let keyName: String
        if let special = KeyMapper.name(for: event.keyCode) {
            keyName = special
        } else if let chars = event.characters, chars.unicodeScalars.count == 1,
                  let scalar = chars.unicodeScalars.first,
                  !CharacterSet.controlCharacters.contains(scalar) {
            keyName = chars
        } else {
            let effects = bridge.flush()
            VntBridge.execute(effects, on: textClient)
            return false
        }

        let effects = bridge.processKey(
            keyName,
            ctrl: ctrl,
            alt: alt,
            meta: meta,
            rawKeycode: UInt32(event.keyCode)
        )
        return VntBridge.execute(effects, on: textClient)
    }

    override func menu() -> NSMenu! {
        let menu = NSMenu(title: "VNType")
        let prefsItem = NSMenuItem(
            title: "Mở cài đặt VNType...",
            action: #selector(openPreferences(_:)),
            keyEquivalent: ","
        )
        prefsItem.target = self
        menu.addItem(prefsItem)

        let macrosItem = NSMenuItem(
            title: "Bảng gõ tắt (macros.txt)...",
            action: #selector(openMacros(_:)),
            keyEquivalent: ""
        )
        macrosItem.target = self
        menu.addItem(macrosItem)
        return menu
    }

    @objc private func openPreferences(_ sender: Any?) {
        PreferencesWindowController.shared.show()
    }

    @objc private func openMacros(_ sender: Any?) {
        let path = Self.defaultConfigURL()?.path
        if VntBridge.ensureMacrosFile(configPath: path), let url = Self.defaultMacrosURL() {
            NSWorkspace.shared.open(url)
        }
    }

    static func defaultConfigURL() -> URL? {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
            .appendingPathComponent("VNType")
            .appendingPathComponent("config.json")
    }

    static func defaultMacrosURL() -> URL? {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
            .appendingPathComponent("VNType")
            .appendingPathComponent("macros.txt")
    }
}
