# Hướng dẫn phát triển VNType

Tài liệu này hướng dẫn lập trình viên thiết lập môi trường phát triển, biên dịch, kiểm thử, gỡ lỗi và đóng gói **VNType** trên **Linux**, **Windows** và **macOS**.

---

## 1. Cấu trúc workspace

```text
vntype/
├── crates/
│   ├── vntype-syllable/       # Âm tiết, thanh điệu, kiểm tra chính tả (hỗ trợ no_std)
│   ├── vntype-core/           # Composer, diff, rebuild, 18 bảng mã (hỗ trợ no_std)
│   ├── vntype-config/         # Lược đồ Config, AppProfile, lưu nguyên tử
│   ├── vntype-settings-core/  # SettingsViewModel & PlatformCapabilities dùng chung
│   ├── vntype-engine/         # ImeSession, macro, dictionary, surrounding port
│   ├── vntype-adapter/        # AdapterController, lập kế hoạch Effect cho IME
│   └── vntype-ffi/            # C ABI (libvntype_ffi) dùng cho Fcitx5 (C++) & macOS (Swift)
├── apps/
│   └── vntype-cli/            # Công cụ dòng lệnh `vntype-cli`
├── platforms/
│   ├── linux/
│   │   ├── ibus/              # Engine IBus thuần Rust (`vntype-ibus`)
│   │   ├── settings/          # Giao diện cấu hình GTK3 thuần Rust (`vntype-linux-settings`)
│   │   └── fcitx5/            # Addon Fcitx5 C++ (`libvntype.so` + CMake + CTest)
│   ├── windows/
│   │   ├── ipc/               # Shared memory nguyên tử (`vntype-win-ipc`)
│   │   ├── hook/              # Hook WH_KEYBOARD_LL + Tray + Single-instance (`vntype-hook`)
│   │   ├── tsf/               # COM In-Process Text Service DLL (`vntype_tsf.dll`)
│   │   ├── settings/          # Giao diện cấu hình Win32 API thuần (`vntype-settings`)
│   │   ├── vntype.ico         # Icon đa kích thước cho `.exe` và thanh tiêu đề
│   │   └── vntype.rc          # Resource script nhúng icon
│   └── macos/                 # InputMethodKit + SwiftUI (`VNTypeIM` + SwiftPM)
├── packaging/
│   ├── linux/                 # `package-deb.sh`, `package-rpm.sh`, `install-source.sh`
│   ├── windows/               # `package-zip.ps1`, `package-tsf.ps1`
│   └── macos/                 # `package-macos.sh`
└── .github/workflows/         # `ci.yml` (CI) & `release.yml` (Release .deb/.rpm/.zip)
```

### Yêu cầu chung

- **Rust**: `stable` (edition `2024`, `rust-version = "1.89"`, cấu hình tại [rust-toolchain.toml](../rust-toolchain.toml) kèm `rustfmt` và `clippy`).
- **just** *(khuyên dùng)*: `cargo install just` để chạy nhanh các tác vụ trong [Justfile](../Justfile).

---

## 2. Thiết lập môi trường & Build theo hệ điều hành

### A. Linux (Debian/Ubuntu, Fedora, Arch)

#### 1. Cài đặt gói hệ thống

- **Debian / Ubuntu**:

  ```bash
  sudo apt update
  sudo apt install -y build-essential cmake pkg-config git extra-cmake-modules \
    libgtk-3-dev libfcitx5core-dev libfcitx5utils-dev libdbus-1-dev dpkg-dev
  ```

- **Fedora**:

  ```bash
  sudo dnf install -y gcc-c++ cmake pkgconf-pkg-config git extra-cmake-modules \
    gtk3-devel fcitx5-devel dbus-devel rpm-build
  ```

- **Arch Linux**:

  ```bash
  sudo pacman -S --needed base-devel cmake pkgconf git extra-cmake-modules \
    gtk3 fcitx5 dbus
  ```

#### 2. Build Rust workspace

```bash
cargo build --workspace
# Hoặc bản release:
cargo build --workspace --release
```

#### 3. Build & kiểm thử Fcitx5 C++ addon

```bash
./platforms/linux/fcitx5/build.sh
# Hoặc thủ công:
cargo build --release -p vntype-ffi
cmake -B platforms/linux/fcitx5/build -S platforms/linux/fcitx5 -DCMAKE_BUILD_TYPE=Release
cmake --build platforms/linux/fcitx5/build -j"$(nproc)"
ctest --test-dir platforms/linux/fcitx5/build --output-on-failure
```

#### 4. Gỡ lỗi realtime cho `vntype-ibus`

`vntype-ibus` tích hợp sẵn ghi log realtime khi biên dịch ở chế độ **debug** (`#[cfg(all(debug_assertions, not(test)))]`) — ở bản **release** phần log này bị loại bỏ hoàn toàn:

1. Đóng gói và cài bản debug:

   ```bash
   ./packaging/linux/package-deb.sh --debug
   sudo dpkg -i dist/linux/vntype-ibus_*_amd64.deb
   ibus restart
   ```

2. Theo dõi từng phím gõ, `SetSurroundingText`, `Reset`, `FocusIn`, và các tín hiệu trả về theo thời gian thực:

   ```bash
   tail -f /tmp/vntype-ibus-debug.log
   ```

3. Kiểm tra nhanh qua `stdin` không cần D-Bus:

   ```bash
   cargo run -p vntype-ibus -- --self-test
   ```

---

### B. Windows (10 / 11)

#### 1. Cài đặt công cụ

- Cài **Rust** (`x86_64-pc-windows-msvc` hoặc `aarch64-pc-windows-msvc`) qua `rustup-init.exe`.
- Cài **Visual Studio Build Tools** với workload **Desktop development with C++** (Windows 10/11 SDK có `rc.exe` + MSVC linker để nhúng `vntype.manifest` chuẩn Windows 11 Visual Styles / `PerMonitorV2` và icon `vntype.ico`).

#### 2. Build & kiểm tra nhanh trên Windows

```powershell
# Kiểm tra toàn workspace:
cargo test --workspace

# Chạy self-test của trình nền Hook:
cargo run -p vntype-hook -- --self-test

# Chạy giao diện cấu hình:
cargo run -p vntype-settings
```

#### 3. Đóng gói & thử nghiệm trên Windows

- **Bản Portable (`VNType.exe`)**:

  ```powershell
  .\packaging\windows\package-zip.ps1
  ```

- **Bản TSF COM DLL (`vntype_tsf.dll`)**:

  ```powershell
  .\packaging\windows\package-tsf.ps1
  # Đăng ký (Admin):
  regsvr32 dist\windows\...\vntype_tsf.dll
  # Gỡ đăng ký (Admin):
  regsvr32 /u vntype_tsf.dll
  ```

---

### C. macOS (Apple Silicon & Intel)

#### 1. Cài đặt công cụ

- Cài **Xcode Command Line Tools** (hoặc Xcode đầy đủ):

  ```bash
  xcode-select --install
  ```

- Cài đủ 2 target Rust để build Universal Binary (`arm64` + `x86_64`):

  ```bash
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  ```

#### 2. Build & kiểm thử trên macOS

```bash
# Kiểm tra phần Rust:
cargo test --workspace

# Build thư viện tĩnh FFI và biên dịch/test phần Swift InputMethodKit:
cargo build --release -p vntype-ffi
cd platforms/macos
swift build
swift test
```

#### 3. Đóng gói `.pkg` và `.dmg`

```bash
./packaging/macos/package-macos.sh
```

*(Có thể truyền `CODESIGN_IDENTITY="..."` nếu ký bằng Developer ID; mặc định dùng ad-hoc `-`.)*

---

## 3. Quy trình kiểm thử & chuẩn mã nguồn (CI)

Chạy đầy đủ các bước kiểm tra trước khi commit:

```bash
just ci
```

Lệnh `just ci` tương đương với:

1. `cargo fmt --all -- --check` — kiểm tra định dạng theo [rustfmt.toml](../rustfmt.toml).
2. `cargo clippy --workspace --all-targets -- -D warnings` — không cho phép bất kỳ warning nào.
3. `cargo check` cho các cấu hình `no_std` của `vntype-syllable` và `vntype-core`.
4. `cargo test --workspace` — toàn bộ unit test, integration test và doctest.
5. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` — kiểm tra tài liệu `rustdoc`.

> Nếu biên dịch trong môi trường không có mạng, thêm `--offline` cho `cargo` (hoặc `VNTYPE_OFFLINE=1` cho các script đóng gói).

### Kiểm thử nhanh bộ gõ bằng CLI

```bash
cargo run -p vntype-cli -- --help
```

---

## 4. Quy tắc phiên bản & Phát hành (Release)

- **Định dạng phiên bản**: VNType sử dụng `{year}.{month}.{number}` (ví dụ `2026.11.0`), cấu hình tập trung tại `[workspace.package]` trong [Cargo.toml](../Cargo.toml). Không dùng số `0` đứng đầu ở bất kỳ thành phần nào để tương thích chuẩn SemVer của Cargo.
- **Nhật ký thay đổi**: Cập nhật [CHANGELOG.md](../CHANGELOG.md) theo chuẩn [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/).
- **Phát hành tự động**: Khi push tag dạng `v*` (ví dụ `git tag v2026.11.0 && git push origin main v2026.11.0`), workflow [release.yml](../.github/workflows/release.yml) sẽ tự động build các gói `.deb`, `.rpm` (Linux) và `.zip` portable (Windows) rồi tạo GitHub Release.

---

## 5. Quy tắc đóng góp

- Giữ phần lõi (`vntype-syllable`, `vntype-core`) tương thích `no_std`.
- Các binding phụ thuộc hệ điều hành cụ thể đặt sau cờ `#[cfg(windows)]` / `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "macos")]` để toàn workspace luôn `cargo check` và `cargo test` được trên mọi máy phát triển.
- Viết unit test đi kèm cho mọi sửa lỗi engine/adapter/platform và chạy `just ci` trước khi tạo commit.
