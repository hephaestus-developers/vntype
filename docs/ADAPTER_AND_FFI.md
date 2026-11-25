# Adapter đa nền tảng & C ABI (`vntype-adapter`, `vntype-ffi`)

Tài liệu này giải thích tầng điều phối trung gian giúp mọi backend (IBus, Fcitx5, Windows Hook/TSF, macOS IMK) hành xử đồng nhất và an toàn.

---

## 1. `vntype-adapter`

Crate nằm tại `crates/vntype-adapter/src/`:

### A. `AdapterController` & `Effect` (`controller.rs`)

Thay vì để từng nền tảng tự đoán cách xử lý sự kiện phím, `AdapterController` nhận `KeyEvent` (kèm `raw_keycode`) và trả về `Vec<Effect>`:

- `Effect::SetPreedit { text, cursor_utf16, cursor_bytes }` — Cập nhật vùng gạch chân tạm thời.
- `Effect::ClearPreedit` — Ẩn vùng gạch chân.
- `Effect::CommitText { text }` — Chốt văn bản vào ứng dụng.
- `Effect::ReplaceText { delete_utf16, delete_chars, delete_bytes, insert }` — Xóa lùi trước con trỏ rồi chèn chuỗi mới.
- `Effect::WaitMs { ms }` — Yêu cầu OS chờ `ms` rồi gọi `on_timer_tick()` (dùng cho `async_replace` 2 pha).
- `Effect::ForwardKey { raw_keycode }` — Trả phím gốc về cho ứng dụng xử lý.

### B. `field_policy.rs`

- Phân loại `FieldKind`: `Text`, `Password`, `Number`, `Email`, `Url`, `Terminal`.
- Ô `Password` (`is_disabled`) luôn đi thẳng phím qua (`PlanKind::Pass`) để tuyệt đối không ghi nhận hay biến đổi mật khẩu.
- Ô `Url` / `Email` / `Number` áp dụng chính sách hạn chế biến đổi tránh làm sai địa chỉ web/email.

### C. `surrounding_guard.rs` & `planner.rs`

- `check_replace` xác minh:
  1. Con trỏ không nằm trong vùng bôi đen (`cursor_utf16 == anchor_utf16`).
  2. Chuỗi trước con trỏ trong `SurroundingSnapshot` khớp hậu tố `before_display`.
- `replace_wait_ms(profile, delete_chars)` tính thời gian trễ an toàn (`MIN_REPLACE_WAIT_MS`..`MAX_REPLACE_WAIT_MS`) dựa trên `AppProfile` khi bật `async_replace`.

### D. `offset.rs`

- Quy đổi chính xác giữa 3 hệ quy chiếu độ dài chuỗi: **UTF-8 bytes**, **Unicode scalar (`char`)** và **UTF-16 code units**.
- Hỗ trợ nhận diện ranh giới từ (`is_word_codepoint`) và trích xuất tiền tố/hậu tố an toàn không cắt đôi surrogate pair.

---

## 2. `vntype-ffi` — Giao diện nhị phân C ABI

Crate nằm tại `crates/vntype-ffi/` cùng header chuẩn tại [crates/vntype-ffi/include/vntype.h](../../crates/vntype-ffi/include/vntype.h).

### Quy ước bộ nhớ & An toàn

- Mọi chuỗi vào/ra dùng **UTF-8** kết thúc `'\0'`.
- Mọi hàm `extern "C"` đều được bọc `catch_unwind` (`guard` trong `handle.rs`) để không bao giờ panic xuyên qua biên FFI.
- Con trỏ `const char *` trong `VntEffect.text` trỏ tới vùng đệm nội bộ của `VntController`, hợp lệ cho đến lần gọi tiếp theo trên cùng `VntController` (lớp gọi C++/Swift **không** cần tự `free`).
- Sinh tự động hoặc đồng bộ bằng `cbindgen.toml`, kiểm soát symbol xuất ra qua `symbols.exports` (macOS) và `symbols.version` (Linux).

### Các nhóm hàm chính trong `vntype.h`

- Vòng đời: `vnt_version()`, `vnt_controller_new(config_json, mode)`, `vnt_controller_free(c)`.
- Cấu hình hành vi: `vnt_controller_set_async_replace`, `vnt_controller_set_allow_unverified_replace`, `vnt_controller_reload_config`, `vnt_controller_set_field_kind`, `vnt_controller_focus_changed`.
- Xử lý phím & ngữ cảnh: `vnt_controller_process_key`, `vnt_controller_set_surrounding_utf16`, `vnt_controller_on_timer_tick`, `vnt_controller_flush`, `vnt_controller_reset`.
- Chuyển đổi bảng mã: các hàm tiện ích chuyển đổi văn bản qua `vntype_core::charset`.
