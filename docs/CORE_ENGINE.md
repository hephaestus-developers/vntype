# Lõi xử lý tiếng Việt (`vntype-syllable`, `vntype-core`, `vntype-engine`)

Tài liệu này mô tả chi tiết cấu trúc dữ liệu, thuật toán biến đổi tiếng Việt và quản lý phiên gõ ở 3 crate lõi.

---

## 1. `vntype-syllable` — Âm vị học & Chính tả (`no_std`)

Crate nằm tại `crates/vntype-syllable/src/`, gồm 4 mô-đun chính:

- **`phonology.rs`**:
  - Phân tích chuỗi chữ cái thành `Syllable { onset, cluster, coda }`:
    - `onset`: phụ âm đầu (`tr`, `ngh`, `gi`, `qu`, ...).
    - `cluster`: cụm nguyên âm (tối đa 3 nguyên âm như `uyê`, `uôi`, `ươi`, ...).
    - `coda`: phụ âm cuối (`p`, `t`, `c`, `ch`, `m`, `n`, `ng`, `nh`).
  - Tổ hợp & phân rã ký tự theo chuẩn Unicode NFC/NFD (`compose_char`, `decompose_char`, `strip_tone`) với `Mark` (`None`, `Circumflex`, `Breve`, `Horn`, `Stroke`).
- **`tone.rs`**:
  - Xác định vị trí đặt dấu thanh (`place_tone`) theo `ToneStyle::New` (*hoà, thuý*) hoặc `ToneStyle::Old` (*hòa, thúy*).
- **`spellcheck.rs`**:
  - Kiểm tra chính tả 3 trạng thái (`Status`):
    - `Valid`: âm tiết tiếng Việt hoàn chỉnh, hợp lệ.
    - `Partial`: tiền tố hợp lệ đang gõ dở (ví dụ `nguy` trước khi thành `nguyên`).
    - `Invalid`: không thể tạo thành âm tiết tiếng Việt hợp lệ (dùng cho `spell_check` và `auto_restore_keys`).
- **`case.rs`**:
  - Chuyển đổi chữ hoa/thường chính xác cho toàn bộ bảng chữ cái tiếng Việt (`lower`, `upper`).

---

## 2. `vntype-core` — Bộ biến đổi & Bảng mã (`no_std`)

Crate nằm tại `crates/vntype-core/src/`:

### A. Strategy kiểu gõ (`rules.rs`)

- Mỗi kiểu gõ được khai báo bằng `InputMethodSpec`:
  - `TELEX`: thanh `s/f/r/x/j`, xóa dấu `z`, dấu phụ `a/e/o/w/d`, phím đứng riêng `w -> ư`, phím ngoặc `[ -> ơ`, `] -> ư`.
  - `SIMPLE_TELEX`: giống Telex nhưng bỏ `standalone` (`w` đứng riêng không thành `ư`) và bỏ `insert_keys` (`[`, `]`).
  - `VNI`: thanh `1..5`, xóa dấu `0`, dấu phụ `6/7/8/9`, `digit_keys = true`.
  - `VIQR`: thanh `' /` / ? / ~ / .`, xóa dấu`0`, dấu phụ`^ / ( / + / d`.
- Hàm `quick_telex(c)` hỗ trợ gõ tắt phụ âm đầu khi bật `quick_telex`: `cc -> ch`, `gg -> gi`, `kk -> kh`, `nn -> ng`, `qq -> qu`, `pp -> ph`, `tt -> th`.

### B. Máy trạng thái `Composer` (`composer.rs`)

- Lưu mảng chữ cái (`MAX_LETTERS`) kèm dấu phụ, dấu thanh, trạng thái viết hoa và bộ đệm phím thô (`MAX_RAW`).
- Khi nhấn một phím:
  1. Kiểm tra phím chức năng theo `InputMethodSpec`.
  2. Nếu phím lặp lại dấu đang có (ví dụ `aa` lần 2 thành `aaa` → `aa`), thực hiện hoàn nguyên dấu và ghi lại ký tự thô.
  3. Hỗ trợ gõ tự do (`free_typing`), `w_as_u`, `bracket_keys`, `allow_coda_k`, và `vni_literal` để tránh biến đổi nhầm số thành dấu ở ngữ cảnh không hợp lệ.
- Khi hết từ mà `Status::Invalid` và bật `auto_restore_keys`, `Composer` khôi phục lại chuỗi phím thô ban đầu (ví dụ từ tiếng Anh như `window`, `github`).

### C. Nghịch đảo & Diff (`rebuild.rs`, `diff.rs`)

- `rebuild` / `to_raw`: Tái tạo `Composer` (cùng chuỗi phím thô ước lượng) từ một chuỗi chữ tiếng Việt đã commit trên màn hình — đây là nền tảng của tính năng **sửa từ trước (`edit_previous_word`)**.
- `diff`: Tính số ký tự cần xóa lùi và chuỗi cần chèn tối thiểu giữa chuỗi hiển thị cũ và mới (dùng cho `OutputMode::Replace`).

### D. Chuyển đổi bảng mã (`charset.rs`, `build.rs`)

- Dữ liệu ánh xạ nằm tại `data/charsets/`, được `crates/vntype-core/build.rs` sinh mã tĩnh lúc biên dịch thành `charset_tables.rs`.
- Hỗ trợ chuyển đổi qua lại (`convert_charset`, `encode_charset`, `decode_charset`, `detect_charset`, `remove_diacritics`) giữa `Unicode NFC/NFD`, `TCVN3`, `VNI-Windows`, `VIQR`, `CP1258`, `VISCII`, `VPS`, `BKHCM1/2`, `Vietware`, v.v.

---

## 3. `vntype-engine` — Phiên gõ & Cổng ứng dụng

Crate nằm tại `crates/vntype-engine/src/`:

- **`session.rs` (`ImeSession`)**:
  - Quản lý trạng thái gõ của một ô nhập liệu: `display_text()`, `is_adopted()`, `field_kind()`, `profile()`, `cap_next`, `pending_end`.
  - Xử lý `KeyEvent`: chữ cái, số, dấu cách, dấu câu, `Backspace` (có thể adopt từ trước nếu `edit_previous_word = true`), `Enter`, `Tab`, `Escape` (hoàn tác/hủy preedit), phím điều hướng.
  - `focus_changed(app_id)`: flush từ đang gõ, đặt lại `field = FieldKind::Text`, nạp `AppProfile` tương ứng với `app_id`.
- **`macro_table.rs`**:
  - Hỗ trợ cả gõ tắt đơn từ (`expand_single_macro`) và cụm từ (`match_phrase_macro`), giữ quy tắc viết hoa thông minh (`py_isupper`: `ko -> không`, `Ko -> Không`, `KO -> KHÔNG`).
- **`dictionary.rs`**:
  - `WordListDictionary` & `shared_dictionary` (Copy-on-Write) phục vụ tra cứu/học từ vựng người dùng.
- **`ports.rs`**:
  - Định nghĩa các trait cổng ra theo Hexagonal Architecture: `ConfigStore`, `AppContext`, `SurroundingText`, `Dictionary` cùng bản cài đặt mặc định (`JsonFileConfigStore`, `MemoryConfigStore`, `StaticAppContext`).
