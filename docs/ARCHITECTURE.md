# Kiến trúc hệ thống (Architecture)

VNType được thiết kế theo **kiến trúc phân tầng / lục giác (Hexagonal Architecture)**, tách biệt hoàn toàn tri thức ngôn ngữ tiếng Việt ở lõi với các giao thức IME đặc thù của từng hệ điều hành ở vòng ngoài.

---

## 1. Sơ đồ phân tầng crate

```text
+-----------------------------------------------------------------------+
|                    Platform Backends & Apps                           |
|  Linux: vntype-ibus (D-Bus), fcitx5 (C++), vntype-linux-settings      |
|  Windows: vntype-hook, vntype-tsf (COM), vntype-settings, vntype-ipc  |
|  macOS: VNTypeIM (Swift/IMK)   |   CLI: vntype-cli                    |
+----------------------------------+------------------------------------+
                                   | (Rust API hoặc C ABI vntype-ffi)
                                   v
+-----------------------------------------------------------------------+
|  vntype-adapter (AdapterController, Plan, surrounding_guard, offset)  |
|  vntype-settings-core (SettingsViewModel, PlatformCapabilities)       |
+----------------------------------+------------------------------------+
                                   |
                                   v
+-----------------------------------------------------------------------+
|  vntype-engine (ImeSession, Ports, MacroTable, Dictionary)            |
|  vntype-config (Config schema, AppProfile, atomic_save, macros.txt)   |
+----------------------------------+------------------------------------+
                                   |
                                   v
+-----------------------------------------------------------------------+
|  vntype-core (Composer, InputMethodSpec, rebuild, diff, charset)      |
|  [no_std + alloc]                                                     |
+----------------------------------+------------------------------------+
                                   |
                                   v
+-----------------------------------------------------------------------+
|  vntype-syllable (phonology, tone, spellcheck, case)                  |
|  [no_std + alloc]                                                     |
+-----------------------------------------------------------------------+
```

### Vai trò từng tầng

1. **`vntype-syllable` (`no_std`)**: Phân tích âm tiết tiếng Việt (`onset`, `cluster`, `coda`), tổ hợp/phân rã Unicode (`compose_char`, `decompose_char`), đặt dấu thanh (`ToneStyle::New` / `Old`) và kiểm tra chính tả (`Status::{Valid, Partial, Invalid}`).
2. **`vntype-core` (`no_std`)**: Bộ biến đổi phím tăng dần (`Composer`), bảng luật Strategy (`InputMethodSpec` cho `Telex`, `Simple Telex`, `VNI`, `VIQR`), phép nghịch đảo khôi phục phím thô (`rebuild`, `to_raw`), tính diff xóa/chèn tối thiểu (`diff`) và chuyển đổi bảng mã (`charset`).
3. **`vntype-config`**: Định nghĩa `Config`, `AppProfile`, chuẩn hóa `app_id`, đọc/lưu cấu hình nguyên tử (`save_config_atomic`) và nạp tệp gõ tắt `macros.txt`.
4. **`vntype-engine`**: Tầng ứng dụng (`ImeSession`), quản lý trạng thái một phiên gõ, tra từ điển (`WordListDictionary`), bung gõ tắt (`macro_table`) và định nghĩa các cổng (`ConfigStore`, `AppContext`, `SurroundingText`, `Dictionary`).
5. **`vntype-adapter`**: Chuẩn hóa giao thức với OS thông qua `AdapterController`, quy đổi offset UTF-8 / UTF-16 / `char` (`offset`), áp dụng chính sách ô nhập nhạy cảm (`field_policy`), xác minh văn bản xung quanh (`surrounding_guard`) và sinh danh sách `Effect` nguyên tử.
6. **`vntype-ffi`**: Phơi bày `AdapterController` và chuyển đổi bảng mã ra C ABI (`include/vntype.h`, `catch_unwind` an toàn) cho Fcitx5 (C++) và macOS (Swift).
7. **`vntype-settings-core`**: Chia sẻ `SettingsViewModel` và `PlatformCapabilities` giữa GTK3 (Linux), Win32 (Windows) và SwiftUI (macOS).

---

## 2. Vòng đời một phím bấm (Keystroke Lifecycle)

```text
OS KeyEvent (keyval/keycode/state)
   │
   ▼
Platform Backend (vd: vntype-ibus::process_key_event)
   │  Ánh xạ sang KeyEvent { key, ctrl, alt, meta } + raw_keycode
   ▼
AdapterController::process_key(ev, raw_keycode)
   │  1. Kiểm tra FieldKind (Password -> Pass ngay)
   │  2. Gọi ImeSession::process_key(&ev)
   ▼
ImeSession
   │  - Xử lý phím chữ / số / dấu / Backspace / Enter / Space / Esc
   │  - Gọi Composer::push(...) hoặc MacroTable / Adopt word
   │  - Trả về Action::{Pass, Preedit, Commit, Replace, FailSafe}
   ▼
AdapterController::translate_action(...)
   │  - Gọi planner::plan_action_with_fallback(...)
   │  - Kiểm tra surrounding_guard nếu cần Replace
   │  - Sinh Vec<Effect>: SetPreedit, ClearPreedit, CommitText,
   │    ReplaceText, WaitMs, ForwardKey
   ▼
Platform Backend (vd: IbusEngineService::apply_effects)
   │  Chuyển Effect thành tín hiệu OS (D-Bus / Win32 SendInput / TSF / IMK)
   ▼
Ứng dụng đích hiển thị chữ
```

---

## 3. Hai chế độ xuất văn bản: `Preedit` vs `Replace`

VNType hỗ trợ hai mô hình `OutputMode` tùy theo năng lực của nền tảng:

1. **`OutputMode::Preedit`** (dùng trên **Linux IBus**, **Linux Fcitx5**, **macOS IMK**):
   - Khi đang gõ dở từ, từ đó nằm trong vùng gạch chân (`UpdatePreeditText`) của hệ thống IME; ứng dụng chưa nhận ký tự thật.
   - Khi nhấn dấu cách, dấu câu, `Enter`, hoặc `FocusOut`, engine phát `CommitText` rồi `HidePreeditText`.
   - **Ưu điểm**: Không gây nhấp nháy xóa-chèn liên tục trên từng phím; ứng dụng không bị rối lịch sử Undo.
2. **`OutputMode::Replace`** (dùng trên **Windows Hook** hoặc app không hỗ trợ preedit):
   - Mọi ký tự được gửi trực tiếp vào ứng dụng; khi cần bỏ dấu/đổi dấu, `diff` tính phần hậu tố thay đổi (`delete_chars` + `insert`).
   - Hỗ trợ giao dịch bất đồng bộ 2 pha (`async_replace = true`): xóa lùi trước → chờ `WaitMs` (tính từ `AppProfile`) → chèn chuỗi mới sau, tránh xung đột thứ tự phím trong Electron/trình duyệt.

### Cơ chế *Adopt Previous Word* & *Fail-Safe*

- **Adopt (`edit_previous_word`)**: Khi người dùng đã commit một từ rồi nhấn `Backspace` (hoặc gõ tiếp phím sửa dấu) ngay sát sau từ đó, adapter đọc chuỗi trước con trỏ từ `SurroundingText`, tách từ cuối cùng và nạp ngược vào `ImeSession` (`is_adopted() = true`) bằng `rebuild`, cho phép sửa dấu tự nhiên mà không cần xóa cả từ.
- **Fail-Safe**: Nếu xảy ra lỗi hoặc chuyển trạng thái đột ngột khi đang ở chế độ `Preedit` và từ chưa từng được commit (`!was_adopted && !before_display.is_empty()`), `AdapterController` bắt buộc phát `Effect::CommitText(before_display)` trước khi `ClearPreedit` và `reset()` để **không bao giờ bị mất chữ người dùng đang gõ**.

---

## 4. Các quyết định kiến trúc (ADRs)

Xem chi tiết lý do đằng sau các lựa chọn thiết kế tại thư mục [adr/](adr/0001-no-std-core.md):

- [ADR-0001: Tách lõi `vntype-syllable` và `vntype-core` theo chuẩn `no_std`](adr/0001-no-std-core.md)
- [ADR-0002: Mô hình Strategy hướng dữ liệu (`InputMethodSpec`) cho các kiểu gõ](adr/0002-data-driven-input-methods.md)
- [ADR-0003: Dùng giao diện Native (Win32 / GTK3 / SwiftUI) thay vì `eframe`](adr/0003-native-ui-over-eframe.md)
- [ADR-0004: Kiểm soát xóa lùi bằng `surrounding_guard` và `OutputMode::Preedit`](adr/0004-surrounding-guard-and-preedit.md)
