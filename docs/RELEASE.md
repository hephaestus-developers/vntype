# Quy trình Đóng gói & Phát hành (Release & Packaging)

VNType sử dụng quy ước phiên bản **`{year}.{month}.{number}`** (ví dụ `2026.11.0`, `2026.11.1`, `2026.11.2`) và phát hành tự động thông qua GitHub Actions khi đẩy tag `v*`.

---

## 1. Quy trình phát hành một phiên bản mới

1. **Kiểm tra toàn bộ chất lượng mã nguồn**:

   ```bash
   just ci
   ```

2. **Bump version** trong [Cargo.toml](../Cargo.toml) (`[workspace.package] -> version = "YYYY.MM.N"`).
3. **Cập nhật `Cargo.lock`** bằng cách chạy kiểm tra:

   ```bash
   cargo test --workspace
   ```

4. **Cập nhật [CHANGELOG.md](../CHANGELOG.md)**:
   - Chuyển các mục mới vào `## [YYYY.MM.N] - YYYY-MM-DD` (tuân theo định dạng *Keep a Changelog*: `Added`, `Changed`, `Fixed`).
   - Cập nhật các link so sánh ở cuối file (`[unreleased]`, `[YYYY.MM.N]`).
5. **Commit và tạo annotated tag**:

   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git commit -m "chore(release): bump version to YYYY.MM.N"
   git tag -a vYYYY.MM.N -m "Release vYYYY.MM.N"
   ```

6. **Đẩy lên GitHub** để kích hoạt workflow phát hành:

   ```bash
   git push origin main --follow-tags
   ```

---

## 2. Cơ chế CI/CD & Đóng gói

### A. Continuous Integration ([`.github/workflows/ci.yml`](../.github/workflows/ci.yml))

- Chạy trên mọi `push` vào `main` và `pull_request` nhắm vào `main`:
  - **`Linux Tests`**: cài phụ thuộc GTK3/CMake và chạy `cargo test --workspace`.
  - **`Windows Tests`**: chạy `cargo test` cho toàn bộ workspace crate + `vntype-settings`, `vntype-hook`.

### B. Release Pipeline ([`.github/workflows/release.yml`](../.github/workflows/release.yml))

Kích hoạt khi có tag `v*` (hoặc chạy thủ công `workflow_dispatch`):

- **`Linux (.deb)`**: chạy `bash packaging/linux/package-deb.sh --release` → xuất `dist/linux/*.deb`.
- **`Linux (.rpm)`**: chạy `bash packaging/linux/package-rpm.sh` → xuất `dist/linux/*.rpm`.
- **`Windows (Portable .zip)`**: chạy `./packaging/windows/package-zip.ps1 -Arch x86_64 -Target x86_64-pc-windows-msvc` → xuất `dist/windows/*.zip`.
- **`Publish GitHub Release`**: gom toàn bộ artifact và tạo GitHub Release (kết hợp cấu hình phân nhóm trong `.github/release.yml`).

### C. Các script đóng gói thủ công khác

- `packaging/linux/install-source.sh`: cài đặt trực tiếp từ mã nguồn trên Linux.
- `packaging/windows/package-tsf.ps1`: đóng gói / đăng ký `vntype_tsf.dll`.
- `packaging/macos/package-macos.sh`: đóng gói bundle `VNTypeIM` trên macOS.
