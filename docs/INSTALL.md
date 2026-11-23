# Hướng dẫn cài đặt VNType

Tài liệu này hướng dẫn cách cài đặt và kích hoạt **VNType** trên **Linux** (theo từng Desktop Environment / Display Server), **Windows** và **macOS**.

---

## 1. Linux

### Tại sao bản đóng gói Linux lại chia thành 2 gói?

Trong thư mục `dist/linux/`, script `package-deb.sh` (và `package-rpm.sh`) tách VNType thành 2 gói độc lập:

| Gói | Thành phần bên trong | Phụ thuộc |
| --- | --- | --- |
| `vntype-fcitx5` | Addon C++ `libvntype.so` và cấu hình Fcitx5 | `fcitx5` |
| `vntype-ibus` | Engine `vntype-ibus` và file mô tả `vntype.xml` | `ibus` |

**Lý do tách gói:**

- Trên Linux có hai framework bộ gõ phổ biến là **IBus** và **Fcitx5**.
- Nếu gộp tất cả vào 1 file `.deb`/`.rpm` duy nhất, người dùng GNOME (đang dùng IBus) sẽ bị ép cài cả `fcitx5`, hoặc người dùng KDE/Fcitx5 sẽ bị ép cài cả `ibus` — dễ gây xung đột bộ gõ và kéo theo nhiều thư viện thừa.
- Hai gói không trùng tệp nào nên có thể cài đặt độc lập hoặc song song; bạn chỉ cần cài **1 gói** phù hợp với Desktop Environment của mình.

---

### Bảng chọn nhanh theo Desktop Environment (DE)

| Desktop Environment / Compositor | Khuyến nghị | Gói cần cài |
| --- | --- | --- |
| **GNOME** (Ubuntu, Debian GNOME, Fedora Workstation) | **IBus** (mặc định của GNOME) | `vntype-ibus` |
| **KDE Plasma** (Wayland hoặc X11) | **Fcitx5** | `vntype-fcitx5` |
| **XFCE / Cinnamon / MATE / LXQt** (X11) | **Fcitx5** hoặc **IBus** | `vntype-fcitx5` (hoặc `vntype-ibus`) |
| **Hyprland / Sway / Niri** (Wayland) | **Fcitx5** | `vntype-fcitx5` |

---

### Cách cài đặt trên Debian / Ubuntu / Linux Mint (`.deb`)

1. **Tạo các gói `.deb`** (nếu build từ mã nguồn):

   ```bash
   ./packaging/linux/package-deb.sh
   ```

2. **Cài đặt gói tương ứng** bằng `apt` (tự động xử lý dependency):
   - **Nếu dùng IBus (GNOME, Ubuntu mặc định):**

     ```bash
     sudo apt install ./dist/linux/vntype-ibus_*.deb
     ```

   - **Nếu dùng Fcitx5 (KDE Plasma, XFCE, Hyprland...):**

     ```bash
     sudo apt install ./dist/linux/vntype-fcitx5_*.deb
     ```

---

### Cách cài đặt trên Fedora / RHEL / openSUSE (`.rpm`)

1. **Build gói `.rpm`** (cần có `rpmbuild` / `rpm-build`):

   ```bash
   ./packaging/linux/package-rpm.sh
   ```

2. **Cài đặt gói tương ứng**:

   ```bash
   # Ví dụ với IBus:
   sudo dnf install ./dist/linux/vntype-ibus-*.rpm

   # Hoặc với Fcitx5:
   sudo dnf install ./dist/linux/vntype-fcitx5-*.rpm
   ```

---

### Cài đặt từ mã nguồn (Arch Linux hoặc distro khác)

Dùng script `install-source.sh` và tắt backend không sử dụng:

```bash
# Ví dụ chỉ cài Fcitx5 vào /usr/local:
./packaging/linux/install-source.sh --prefix /usr/local --without-ibus

# Hoặc chỉ cài IBus:
./packaging/linux/install-source.sh --prefix /usr/local --without-fcitx5

# Gỡ cài đặt:
./packaging/linux/install-source.sh --prefix /usr/local --uninstall
```

---

### Kích hoạt bộ gõ sau khi cài trên từng DE

#### A. GNOME (IBus)

1. Khởi động lại IBus:

   ```bash
   ibus restart
   ```

2. Mở **Settings → Keyboard → Input Sources → Add Input Source (`+`) → Vietnamese → VNType**.
3. Dùng phím tắt chuyển bộ gõ của GNOME (mặc định `Super + Space`) để chuyển sang **VNType**.

#### B. KDE Plasma (Fcitx5)

1. Cài đặt gói `fcitx5` và `fcitx5-configtool` (nếu chưa có).
2. Trên **KDE Wayland**: mở **System Settings → Keyboard → Virtual Keyboard**, chọn **Fcitx 5**.
3. Trên **KDE X11** (hoặc app XWayland): thêm vào `~/.profile` hoặc `/etc/environment`:

   ```sh
   GTK_IM_MODULE=fcitx
   QT_IM_MODULE=fcitx
   XMODIFIERS=@im=fcitx
   ```

4. Đăng xuất rồi đăng nhập lại (hoặc chạy `fcitx5 -r -d`).
5. Mở **Fcitx5 Configuration** (`fcitx5-configtool`) → tìm **VNType** ở cột phải và chuyển sang danh sách bộ gõ đang dùng.

#### C. XFCE / Cinnamon / MATE (X11)

- **Nếu dùng Fcitx5**: chạy `im-config -n fcitx5` (trên Debian/Ubuntu) hoặc đặt biến môi trường `GTK_IM_MODULE=fcitx`, `QT_IM_MODULE=fcitx`, `XMODIFIERS=@im=fcitx`, đăng xuất/đăng nhập lại rồi thêm **VNType** trong `fcitx5-configtool`.
- **Nếu dùng IBus**: chạy `im-config -n ibus` (trên Debian/Ubuntu), chạy `ibus-setup` → tab **Input Method** → **Add → Vietnamese → VNType**.

#### D. Hyprland / Sway / wlroots (Wayland)

- **Với Fcitx5** (khuyên dùng nếu cần gõ ổn định trên cả Electron/XWayland/Firefox):
  - Thêm `exec-once = fcitx5 -d` vào cấu hình compositor (`hyprland.conf` hoặc `sway/config`).
  - Đặt `XMODIFIERS=@im=fcitx` (và `QT_IM_MODULE=fcitx`, với GTK/Electron dùng Wayland text-input).
  - Thêm **VNType** trong `fcitx5-configtool`.

#### Cấu hình trên Linux

Trên IBus/GNOME, bạn cấu hình trực tiếp qua menu **VNType** trên thanh Top Bar (Kiểu gõ, Bảng mã, Kiểu bỏ dấu, Tuỳ chọn bộ gõ, Gõ tắt). Ngoài ra có thể dùng CLI:

```bash
vntype-cli --help
```

---

## 2. Windows (10 / 11)

VNType hỗ trợ 2 hình thức chạy trên Windows:

### Cách 1: Bản Portable (`VNType.exe` — Khuyên dùng, không cần Admin)

1. Build gói portable trên Windows bằng PowerShell:

   ```powershell
   .\packaging\windows\package-zip.ps1
   ```

   Tệp đầu ra: `dist\windows\vntype-<version>-windows-<arch>-portable.zip`.
2. Giải nén và chạy **`VNType.exe`** trực tiếp.
3. Sử dụng:
   - Click chuột trái vào biểu tượng **V / E** ở khay hệ thống (hoặc nhấn **`Alt + ,`**) để chuyển Tiếng Việt / Tiếng Anh.
   - Click chuột phải vào biểu tượng khay để đổi kiểu gõ (Telex, VNI, Simple Telex) hoặc mở cửa sổ cấu hình.

### Cách 2: Bản TSF COM DLL (`vntype_tsf.dll` — Tích hợp thanh ngôn ngữ Windows)

1. Build gói TSF trên Windows bằng PowerShell:

   ```powershell
   .\packaging\windows\package-tsf.ps1
   ```

   Tệp đầu ra: `dist\windows\vntype-<version>-windows-<arch>-tsf.zip`.
2. Giải nén, mở **Command Prompt / PowerShell với quyền Administrator** tại thư mục vừa giải nén và chạy:

   ```bat
   regsvr32 vntype_tsf.dll
   ```

3. Chọn **VNType (Tiếng Việt)** trên thanh ngôn ngữ Windows (`Win + Space`).
4. Để gỡ đăng ký:

   ```bat
   regsvr32 /u vntype_tsf.dll
   ```

---

## 3. macOS (Apple Silicon & Intel — Universal)

1. **Đóng gói trên macOS** (yêu cầu Xcode Command Line Tools và Rust với cả 2 target `aarch64-apple-darwin` & `x86_64-apple-darwin`):

   ```bash
   ./packaging/macos/package-macos.sh
   ```

   Tạo ra 2 tệp trong `dist/macos/`:
   - `VNType-<version>-universal.pkg`
   - `VNType-<version>-universal.dmg`

2. **Cài đặt**:
   - **Dùng file `.pkg`**: mở file `.pkg` và làm theo trình cài đặt (tự động cài `VNType.app` vào `/Library/Input Methods/`).
   - **Hoặc dùng file `.dmg`**: mở `.dmg` và kéo `VNType.app` vào thư mục `Input Methods` (`/Library/Input Methods/`).

3. **Kích hoạt bộ gõ**:
   - Mở **System Settings → Keyboard → Text Input → Input Sources → Edit... → `+` → Vietnamese → VNType**.
   - Nếu VNType chưa xuất hiện ngay trong danh sách, hãy **đăng xuất (Log Out) và đăng nhập lại** để macOS nạp lại danh sách Input Method từ `/Library/Input Methods/`.
   - Chuyển bộ gõ bằng tổ hợp phím chuyển Input Source của macOS (mặc định `Ctrl + Space` hoặc phím `Globe`/`Fn`).
