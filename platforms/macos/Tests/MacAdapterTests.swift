import XCTest
@testable import VNTypeIM

/// Đối tượng giả lập `MacTextClient` để kiểm thử không cần giao diện đồ hoạ trên macOS.
///
/// Đặc biệt kiểm tra trường hợp `FailSafe` ở chế độ `Preedit` đảm bảo chữ đang soạn được chốt
/// (`commitText`) trước khi xoá vùng gạch chân (`clearMarkedText`).
final class FakeMacTextClient: MacTextClient {
    var committed: String = ""
    var marked: String = ""
    var cursorUTF16: Int = 0

    func setMarkedText(_ text: String, cursorUTF16: Int) {
        self.marked = text
        self.cursorUTF16 = cursorUTF16
    }

    func clearMarkedText() {
        self.marked = ""
        self.cursorUTF16 = 0
    }

    func commitText(_ text: String) {
        self.committed += text
    }

    func replaceText(_ text: String, deleteBeforeUTF16: Int) {
        let ns = self.committed as NSString
        let keep = max(0, ns.length - deleteBeforeUTF16)
        self.committed = ns.substring(to: keep) + text
    }
}

final class MacAdapterTests: XCTestCase {
    func testPreeditTypingAndCommit() {
        let bridge = VntBridge(mode: 0)
        let client = FakeMacTextClient()

        for ch in ["t", "i", "e", "e", "n", "g", "s"] {
            let consumed = VntBridge.execute(bridge.processKey(ch), on: client)
            XCTAssertTrue(consumed)
        }
        XCTAssertEqual(client.marked, "tiếng")
        XCTAssertEqual(bridge.displayText(), "tiếng")

        let spaceConsumed = VntBridge.execute(bridge.processKey(" "), on: client)
        XCTAssertTrue(spaceConsumed)
        XCTAssertEqual(client.committed, "tiếng ")
        XCTAssertEqual(client.marked, "")
    }

    func testApplyConfigJSONSwitchesToVni() {
        let bridge = VntBridge(mode: 0)
        let client = FakeMacTextClient()

        let patchEffects = bridge.applyConfigJSON(#"{"input_method":"vni"}"#)
        XCTAssertNotNil(patchEffects)

        for ch in ["V", "i", "e", "6", "t", "5", " "] {
            VntBridge.execute(bridge.processKey(ch), on: client)
        }
        XCTAssertEqual(client.committed, "Việt ")
    }

    func testExecuteForwardKeyReturnsFalse() {
        let client = FakeMacTextClient()
        let consumed = VntBridge.execute(
            [VntSwiftEffect(kind: .forwardKey, deleteUTF16: 0, cursorUTF16: 0, waitMs: 0, text: "")],
            on: client
        )
        XCTAssertFalse(consumed)
    }
}
