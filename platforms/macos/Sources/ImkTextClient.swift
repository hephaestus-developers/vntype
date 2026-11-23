import Foundation
import InputMethodKit

/// Lớp bọc đối tượng `IMKTextInput` của macOS sang giao thức [`MacTextClient`].
final class ImkTextClient: MacTextClient {
    private weak var client: AnyObject?

    init(_ client: AnyObject) {
        self.client = client
    }

    func setMarkedText(_ text: String, cursorUTF16: Int) {
        guard let c = client as? IMKTextInput else { return }
        let len = (text as NSString).length
        let clamped = max(0, min(cursorUTF16, len))
        // Luôn truyền NSAttributedString để tương thích với Apple Notes và NSTextView rich text
        c.setMarkedText(
            NSAttributedString(string: text),
            selectionRange: NSRange(location: clamped, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0)
        )
    }

    func clearMarkedText() {
        guard let c = client as? IMKTextInput else { return }
        c.setMarkedText(
            NSAttributedString(string: ""),
            selectionRange: NSRange(location: 0, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0)
        )
    }

    func commitText(_ text: String) {
        guard let c = client as? IMKTextInput else { return }
        c.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
    }

    func replaceText(_ text: String, deleteBeforeUTF16 count: Int) {
        guard let c = client as? IMKTextInput else { return }
        let selected = c.selectedRange()
        guard selected.location != NSNotFound else {
            c.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
            return
        }
        let location = max(0, selected.location - max(0, count))
        let length = min(max(0, count), selected.location)
        c.insertText(text, replacementRange: NSRange(location: location, length: length))
    }

    /// Trích xuất ảnh chụp nhanh văn bản xung quanh con trỏ cho `vntype-adapter`.
    func surroundingSnapshot(maxLength: Int = 25) -> (text: String, cursorUTF16: UInt32, anchorUTF16: UInt32)? {
        guard let c = client as? IMKTextInput else { return nil }
        let selected = c.selectedRange()
        guard selected.location != NSNotFound else { return nil }

        let start = max(0, selected.location - maxLength)
        let beforeLen = max(0, selected.location - start)
        let before = beforeLen > 0
            ? c.attributedSubstring(from: NSRange(location: start, length: beforeLen))?.string ?? ""
            : ""
        let selectedText = selected.length > 0
            ? c.attributedSubstring(from: selected)?.string ?? ""
            : ""
        let afterLoc = selected.location + selected.length
        let after = c.attributedSubstring(from: NSRange(location: afterLoc, length: 1))?.string ?? ""

        let full = before + selectedText + after
        let cursor = (before as NSString).length + (selectedText as NSString).length
        let anchor = (before as NSString).length
        return (full, UInt32(cursor), UInt32(anchor))
    }
}
