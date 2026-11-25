# Kiểm thử, Benchmark & Gỡ lỗi (Testing & Debugging)

Tài liệu này mô tả chiến lược kiểm thử nhiều tầng, cách chạy benchmark và hướng dẫn gỡ lỗi thực tế cho VNType.

---

## 1. Hệ thống kiểm thử tự động

Toàn bộ các lệnh chuẩn được định nghĩa trong [Justfile](../Justfile):

| Lệnh | Mục đích |
| :--- | :--- |
| `just lint` | Kiểm tra format (`cargo fmt --all -- --check`) và Clippy (`-D warnings`) trên toàn workspace |
| `just check` | Kiểm tra biên dịch mặc định và cấu hình `no_std` của `vntype-syllable`, `vntype-core` |
| `just test` | Chạy toàn bộ unit test, integration test và doctest (`cargo test --workspace`) |
| `just doc` | Kiểm tra `rustdoc` không có broken link hay cảnh báo (`RUSTDOCFLAGS="-D warnings"`) |
| `just ci` | Chạy tuần tự `lint`, `check`, `test`, `doc` giống quy trình CI |

### Phân tầng kiểm thử

1. **Unit & Integration test theo từng crate**:
   - `vntype-syllable`: phân tách âm tiết, quy tắc dấu mới/cũ, kiểm tra chính tả (`tests/syllable_flow.rs`).
   - `vntype-core`: biến đổi Telex/VNI/VIQR, hoàn nguyên phím, `rebuild`, `diff`, 16+ bảng mã (`tests/typing_flow.rs`).
   - `vntype-engine`: luồng `ImeSession` (Preedit/Replace, gõ tắt, `adopt_previous_word`, `focus_changed` reset `FieldKind`, `tests/session_flow.rs`).
   - `vntype-adapter`: `surrounding_guard`, quy đổi UTF-8/UTF-16/`char`, `FailSafe` (`tests/adapter_flow.rs`).
   - `vntype-ffi`: kiểm tra C ABI (`tests/ffi_flow.rs`).
   - `vntype-ibus`: kiểm tra xử lý phím D-Bus, thứ tự FIFO `Enter`, `SetSurroundingText`, `held_backspace`, `macros.txt`.
2. **Fcitx5 C++ tests**:

   ```bash
   ctest --test-dir platforms/linux/fcitx5/build --output-on-failure
   ```

3. **macOS Swift tests**:
   - Chạy test tại `platforms/macos/Tests/MacAdapterTests.swift` thông qua `swift test` trong `platforms/macos/`.

---

## 2. Benchmark hiệu năng

VNType sử dụng **Criterion** cho các crate lõi (`vntype-syllable`, `vntype-core`, `vntype-engine`, `vntype-adapter`):

```bash
just bench
# Hoặc chạy riêng từng crate:
cargo bench -p vntype-core
```

---

## 3. Gỡ lỗi thực tế trên Linux (`vntype-ibus`)

Khi build ở chế độ **debug** (`debug_assertions` và không phải `test`), `vntype-ibus` ghi log realtime vào `/tmp/vntype-ibus-debug.log` (bản **release** loại bỏ hoàn toàn macro `debug_log!` để đạt hiệu năng tối đa).

```bash
# Cài bản debug và khởi động lại IBus
bash packaging/linux/package-deb.sh
sudo dpkg -i dist/linux/vntype_*.deb
ibus restart

# Theo dõi log realtime
tail -f /tmp/vntype-ibus-debug.log
```

Các dòng log chính cần chú ý:

- `KEY keyval=... -> handled=... display=... adopted=... signals=[...]`: kiểm tra xem phím có bị xử lý lặp hay thiếu signal không.
- `SET_SURROUNDING raw_bytes=... window=... cursor=...`: kiểm tra chuỗi surrounding xung quanh con trỏ.
- `FOCUS_IN` / `FOCUS_OUT` / `RESET`: theo dõi các đợt chuyển focus của desktop/app.
