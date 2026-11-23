import Foundation

/// Bộ theo dõi tệp cấu hình duy nhất trong toàn tiến trình (Singleton) sử dụng sự kiện nhân `kqueue`
/// (`DispatchSourceFileSystemObject`).
///
/// Khắc phục hoàn toàn nhược điểm của thiết kế cũ (mỗi `IMKInputController` tạo một `Timer` lặp lại
/// mỗi 1 giây gây tiêu tốn tài nguyên khi có nhiều cửa sổ): bộ theo dõi này tiêu thụ 0% CPU khi nghỉ
/// và phát thông báo tức thì ngay khi tệp `config.json` được ghi nguyên tử.
final class ConfigWatcher {
    /// Thực thể dùng chung duy nhất trong toàn ứng dụng.
    static let shared = ConfigWatcher()

    /// Tên thông báo phát qua `NotificationCenter` khi tệp cấu hình thay đổi.
    static let configDidChangeNotification = Notification.Name("VNTypeConfigDidChangeNotification")

    private var source: DispatchSourceFileSystemObject?
    private let queue = DispatchQueue(label: "dev.hephaestus.vntype.configwatcher", qos: .utility)

    private init() {}

    /// Bắt đầu theo dõi thư mục chứa tệp cấu hình `configURL` bằng `kqueue`.
    func startWatching(configURL: URL) {
        stopWatching()
        let directoryURL = configURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: directoryURL, withIntermediateDirectories: true)

        let fd = open(directoryURL.path, O_EVTONLY)
        guard fd >= 0 else { return }

        let src = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .rename, .delete],
            queue: queue
        )
        src.setEventHandler {
            DispatchQueue.main.async {
                NotificationCenter.default.post(name: Self.configDidChangeNotification, object: nil)
            }
        }
        // Capture fd cục bộ để tránh đóng nhầm file descriptor mới khi startWatching được gọi lại liên tiếp
        src.setCancelHandler {
            close(fd)
        }
        source = src
        src.resume()
    }

    /// Dừng theo dõi tệp cấu hình.
    func stopWatching() {
        source?.cancel()
        source = nil
    }
}
