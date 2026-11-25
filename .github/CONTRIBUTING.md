# Hướng dẫn đóng góp cho VNType

Cảm ơn bạn đã quan tâm đóng góp cho **VNType**! Tài liệu này tóm tắt quy trình làm việc; để tìm hiểu sâu về kiến trúc các crate và cách đóng gói từng nền tảng, vui lòng xem thêm [docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md).

## 1. Chuẩn bị môi trường

- Cài đặt Rust toolchain theo [rust-toolchain.toml](../rust-toolchain.toml) (tối thiểu Rust `1.89`, Edition `2024`).
- Cài đặt [`just`](https://github.com/casey/just) (khuyến nghị) để chạy các tác vụ kiểm tra nhanh.
- Trên Linux, cần các thư viện phát triển GTK3/GLib/D-Bus nếu build `vntype-linux-settings` và `vntype-ibus` (xem chi tiết trong [docs/DEVELOPMENT.md](../docs/DEVELOPMENT.md)).

## 2. Quy trình phát triển & kiểm tra

Trước khi tạo Pull Request, hãy đảm bảo mã nguồn vượt qua toàn bộ bộ kiểm tra giống CI:

```bash
just ci
```

Lệnh trên lần lượt chạy:

- `just lint`: `cargo fmt --all -- --check` và `cargo clippy --workspace --all-targets -- -D warnings`
- `just check`: kiểm tra biên dịch workspace và các cấu hình `no_std` của `vntype-syllable`, `vntype-core`
- `just test`: `cargo test --workspace`
- `just doc`: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`

Để tự động format code:

```bash
just fmt
```

## 3. Quy ước Commit

Dự án sử dụng **Conventional Commits**:

- `feat(scope): ...` — Tính năng mới
- `fix(scope): ...` — Sửa lỗi
- `perf(scope): ...` — Tối ưu hiệu năng
- `refactor(scope): ...` — Tái cấu trúc mã nguồn
- `docs(scope): ...` — Tài liệu
- `chore(scope): ...` / `ci(scope): ...` — Cấu hình, CI, release

Ví dụ: `fix(ibus): skip redundant HidePreeditText on adopted backspace`.

## 4. Gửi Pull Request

- Mỗi PR nên tập trung vào một mục tiêu rõ ràng.
- Bổ sung unit test hoặc integration test cho thay đổi ở tầng lõi (`vntype-syllable`, `vntype-core`, `vntype-engine`, `vntype-adapter`) hoặc nền tảng (`vntype-ibus`, v.v.).
- Điền đầy đủ thông tin theo mẫu trong [PULL_REQUEST_TEMPLATE.md](./PULL_REQUEST_TEMPLATE.md).
