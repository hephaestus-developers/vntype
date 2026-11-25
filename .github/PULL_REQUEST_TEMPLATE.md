## Mô tả thay đổi

<!-- Tóm tắt ngắn gọn thay đổi trong PR này và lý do -->

## Loại thay đổi

- [ ] Sửa lỗi (`fix`)
- [ ] Tính năng mới (`feat`)
- [ ] Tối ưu hiệu năng / tái cấu trúc (`perf` / `refactor`)
- [ ] Tài liệu / Đóng gói / CI (`docs` / `build` / `ci`)

## Nền tảng đã kiểm thử

- [ ] Linux (IBus / Fcitx5)
- [ ] Windows (Hook / TSF / Settings)
- [ ] macOS (IMK / Settings)
- [ ] Lõi (`vntype-syllable` / `vntype-core` / `vntype-engine` / `vntype-adapter`)

## Checklist trước khi merge

- [ ] Đã chạy `just lint` (`cargo fmt --all -- --check` và `cargo clippy --workspace --all-targets -- -D warnings`)
- [ ] Đã chạy `just test` (`cargo test --workspace`) và toàn bộ test đều pass
- [ ] Đã cập nhật test mới (nếu sửa lỗi hoặc thêm tính năng)
- [ ] Đã cập nhật `CHANGELOG.md` hoặc tài liệu liên quan (nếu cần)
