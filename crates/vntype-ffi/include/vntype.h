/**
 * @file vntype.h
 * @brief Giao diện nhị phân C ABI chuẩn công nghiệp của bộ gõ tiếng Việt VNType.
 *
 * Quy ước sử dụng:
 * - Mọi chuỗi ký tự đầu vào và đầu ra đều mã hoá UTF-8, kết thúc bằng ký tự NUL ('\0').
 * - Toàn bộ hiệu ứng giao diện trả về dưới dạng danh sách nguyên tử `VntEffectList`.
 * - Lớp vỏ hệ điều hành (C++ Fcitx5, Swift macOS IMK) không cần tự xử lý Fail-Safe hay
 *   tự đổi đơn vị UTF-16/UTF-8: chỉ cần duyệt mảng `effects[0..count]` và thực thi tuần tự.
 * - Mọi con trỏ `const char *` trong `VntEffect` thuộc vùng đệm nội bộ của `VntController`,
 *   hợp lệ cho tới lần gọi hàm tiếp theo trên cùng `VntController` (không cần gọi free).
 */

#ifndef VNTYPE_H
#define VNTYPE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Chế độ xuất chữ: hiển thị vùng gạch chân tạm thời (Preedit / Marked Text). */
#define VNT_MODE_PREEDIT 0u
/** Chế độ xuất chữ: chèn trực tiếp và xoá lùi khi sửa dấu (Replace). */
#define VNT_MODE_REPLACE 1u

/** Phân loại trường nhập liệu: ô văn bản thông thường. */
#define VNT_FIELD_TEXT 0u
/** Phân loại trường nhập liệu: ô mật khẩu hoặc mã PIN nhạy cảm. */
#define VNT_FIELD_PASSWORD 1u
/** Phân loại trường nhập liệu: ô chỉ nhập số. */
#define VNT_FIELD_NUMBER 2u
/** Phân loại trường nhập liệu: ô nhập địa chỉ thư điện tử. */
#define VNT_FIELD_EMAIL 3u
/** Phân loại trường nhập liệu: thanh địa chỉ trình duyệt / Omnibox. */
#define VNT_FIELD_URL 4u
/** Phân loại trường nhập liệu: giao diện dòng lệnh Terminal. */
#define VNT_FIELD_TERMINAL 5u

/** Loại hiệu ứng nguyên tử: đặt chuỗi gạch chân tạm thời (Preedit). */
#define VNT_EFFECT_SET_PREEDIT 1u
/** Loại hiệu ứng nguyên tử: xoá vùng gạch chân tạm thời. */
#define VNT_EFFECT_CLEAR_PREEDIT 2u
/** Loại hiệu ứng nguyên tử: chốt chuỗi văn bản chính thức vào ứng dụng. */
#define VNT_EFFECT_COMMIT_TEXT 3u
/** Loại hiệu ứng nguyên tử: xoá lùi văn bản trước con trỏ và chèn chuỗi mới. */
#define VNT_EFFECT_REPLACE_TEXT 4u
/** Loại hiệu ứng nguyên tử: yêu cầu hệ điều hành hẹn giờ chờ `wait_ms` rồi gọi `vnt_controller_on_timer_tick`. */
#define VNT_EFFECT_WAIT_MS 5u
/** Loại hiệu ứng nguyên tử: chuyển tiếp sự kiện phím vật lý gốc về cho ứng dụng. */
#define VNT_EFFECT_FORWARD_KEY 6u

/** Con trỏ mờ đại diện cho một bộ điều khiển phiên gõ VNType. */
typedef struct VntController VntController;

/**
 * @brief Cấu trúc biểu diễn một hiệu ứng nguyên tử mà hệ điều hành cần thực thi.
 */
typedef struct VntEffect {
    /** Loại hiệu ứng (`VNT_EFFECT_*`). */
    uint32_t kind;
    /** Số đơn vị mã UTF-16 cần xoá lùi (dùng cho `VNT_EFFECT_REPLACE_TEXT`). */
    uint32_t delete_utf16;
    /** Số ký tự Unicode (`char`) cần xoá lùi (dùng cho `VNT_EFFECT_REPLACE_TEXT`). */
    uint32_t delete_chars;
    /** Số byte UTF-8 cần xoá lùi (dùng cho `VNT_EFFECT_REPLACE_TEXT`). */
    uint32_t delete_bytes;
    /** Vị trí con trỏ theo đơn vị mã UTF-16 (dùng cho `VNT_EFFECT_SET_PREEDIT`). */
    uint32_t cursor_utf16;
    /** Vị trí con trỏ theo số byte UTF-8 (dùng cho `VNT_EFFECT_SET_PREEDIT`). */
    uint32_t cursor_bytes;
    /** Thời gian chờ tính bằng mili-giây (dùng cho `VNT_EFFECT_WAIT_MS`). */
    uint32_t wait_ms;
    /** Mã phím vật lý hoặc định danh sự kiện phím gốc (dùng cho `VNT_EFFECT_FORWARD_KEY`). */
    uint32_t raw_keycode;
    /** Chuỗi văn bản UTF-8 kết thúc NUL (dùng cho `SET_PREEDIT`, `COMMIT_TEXT`, `REPLACE_TEXT`). */
    const char *text;
} VntEffect;

/**
 * @brief Danh sách các hiệu ứng nguyên tử trả về sau một thao tác trên `VntController`.
 */
typedef struct VntEffectList {
    /** Con trỏ tới mảng hiệu ứng (NULL nếu `count == 0`). */
    const VntEffect *effects;
    /** Số lượng phần tử trong mảng `effects`. */
    uint32_t count;
} VntEffectList;

/** Trả về chuỗi phiên bản của thư viện VNType. */
const char *vnt_version(void);

/**
 * @brief Khởi tạo một bộ điều khiển phiên gõ mới.
 * @param config_json Chuỗi JSON cấu hình (có thể NULL để dùng mặc định).
 * @param mode Chế độ xuất chữ (`VNT_MODE_PREEDIT` hoặc `VNT_MODE_REPLACE`).
 * @return Con trỏ `VntController*` hoặc NULL nếu JSON sai cú pháp.
 */
VntController *vnt_controller_new(const char *config_json, uint32_t mode);

/** Giải phóng bộ nhớ của một `VntController`. */
void vnt_controller_free(VntController *c);

/** Bật hoặc tắt chế độ tách thao tác `Replace` thành giao dịch bất đồng bộ 2 pha có hẹn giờ. */
void vnt_controller_set_async_replace(VntController *c, bool enabled);

/** Bật hoặc tắt chế độ cho phép `Replace` trực tiếp không cần xác minh surrounding text. */
void vnt_controller_set_allow_unverified_replace(VntController *c, bool allow);

/**
 * @brief Xử lý một sự kiện bàn phím theo tên phím chuẩn ("a", "Backspace", "Enter", "Escape", ...).
 */
VntEffectList vnt_controller_process_key(VntController *c, const char *key, bool ctrl, bool alt,
                                         bool meta, uint32_t raw_keycode);

/**
 * @brief Xử lý một sự kiện bàn phím theo mã điểm Unicode (`codepoint`).
 */
VntEffectList vnt_controller_process_char(VntController *c, uint32_t codepoint, bool ctrl, bool alt,
                                          bool meta, uint32_t raw_keycode);

/**
 * @brief Được gọi khi bộ hẹn giờ `VNT_EFFECT_WAIT_MS` hoàn tất để chốt chữ và xả hàng đợi phím.
 */
VntEffectList vnt_controller_on_timer_tick(VntController *c);

/** Thông báo ứng dụng nhận tiêu điểm đã thay đổi (`app_id` có thể NULL). */
VntEffectList vnt_controller_focus_changed(VntController *c, const char *app_id);

/** Cập nhật ảnh chụp nhanh văn bản xung quanh con trỏ với toạ độ theo đơn vị mã UTF-16. */
void vnt_controller_set_surrounding_utf16(VntController *c, const char *text, uint32_t cursor_utf16,
                                          uint32_t anchor_utf16);

/** Cập nhật văn bản nằm ngay trước con trỏ (dành cho nền tảng chỉ lấy được tiền tố trước con trỏ). */
void vnt_controller_set_surrounding_before(VntController *c, const char *before_cursor);

/** Cập nhật phân loại trường nhập liệu (`VNT_FIELD_*`). */
VntEffectList vnt_controller_set_field_kind(VntController *c, uint32_t field_kind);

/** Thông báo con trỏ soạn thảo đã bị dịch chuyển do nhấp chuột hoặc thao tác bên ngoài. */
VntEffectList vnt_controller_cursor_moved(VntController *c);

/** Đảo trạng thái bật/tắt gõ tiếng Việt. */
VntEffectList vnt_controller_toggle_vietnamese(VntController *c);

/** Chốt vùng văn bản đang soạn dở trước khi đóng hoặc mất tiêu điểm. */
VntEffectList vnt_controller_flush(VntController *c);

/** Xoá sạch trạng thái từ đang gõ và hàng đợi mà không phát sinh hiệu ứng giao diện. */
void vnt_controller_reset(VntController *c);

/** Thay đổi chế độ xuất chữ (`VNT_MODE_PREEDIT` hoặc `VNT_MODE_REPLACE`). */
VntEffectList vnt_controller_set_output_mode(VntController *c, uint32_t mode);

/** Kiểm tra xem bộ gõ tiếng Việt có đang bật hay không. */
bool vnt_controller_is_enabled(VntController *c);

/** Kiểm tra xem bộ điều khiển có đang bận trong một giao dịch `Replace` bất đồng bộ hay không. */
bool vnt_controller_is_busy(VntController *c);

/** Trả về chuỗi văn bản mà engine tin là đang hiển thị cho từ hiện tại. */
const char *vnt_controller_display_text(VntController *c);

/**
 * @brief Áp dụng một mảnh JSON cấu hình mới lên bộ điều khiển.
 * @return `true` nếu JSON hợp lệ (`out_effects` nhận danh sách hiệu ứng cần thực thi trước khi gõ tiếp).
 */
bool vnt_controller_apply_config_json(VntController *c, const char *json, VntEffectList *out_effects);

/** Trả về chuỗi JSON cấu hình hiện hành của phiên. */
const char *vnt_controller_config_json(VntController *c);

/** Trả về số thứ tự phiên bản cấu hình nội bộ (tăng mỗi khi engine tự lưu `app_modes`). */
uint64_t vnt_controller_config_version(VntController *c);

/** Trả về chuỗi JSON của `AppProfile` đang áp dụng cho ứng dụng hiện hành. */
const char *vnt_controller_app_profile_json(VntController *c);

/** Trả về chế độ xuất chữ gợi ý của `AppProfile` hiện hành: 0 = không đặt, 1 = PREEDIT, 2 = REPLACE. */
uint32_t vnt_controller_profile_output_mode(VntController *c);

/** Thêm danh sách từ (phân cách bằng khoảng trắng) vào từ điển của phiên hiện tại. */
void vnt_controller_add_dictionary_words(VntController *c, const char *words, bool vietnamese);

/** Nạp mới toàn bộ từ điển tiếng Anh hoặc tiếng Việt cho phiên hiện tại. */
void vnt_controller_load_dictionary(VntController *c, const char *english, const char *vietnamese);

/** Đổi chỉ số ký tự (`char`) sang chỉ số đơn vị mã UTF-16 (`UINT32_MAX` nếu lỗi). */
uint32_t vnt_char_to_utf16_offset(const char *text, uint32_t chars);

/** Đổi chỉ số ký tự (`char`) sang chỉ số byte UTF-8 (`UINT32_MAX` nếu lỗi). */
uint32_t vnt_char_to_byte_offset(const char *text, uint32_t chars);

/** Đổi chỉ số đơn vị mã UTF-16 sang chỉ số byte UTF-8 (`UINT32_MAX` nếu lỗi). */
uint32_t vnt_utf16_to_byte_offset(const char *text, uint32_t units);

/** Kiểm tra một mã điểm Unicode có phải là ký tự thuộc từ hay không. */
bool vnt_is_word_codepoint(uint32_t codepoint);

/** Trả về danh sách bảng mã hỗ trợ, mỗi dòng có định dạng `"key\tTên hiển thị\n"`. */
const char *vnt_charset_list(void);

/** Mã hoá chuỗi Unicode sang bảng mã đích. */
const char *vnt_charset_encode(VntController *c, const char *charset, const char *text);

/** Giải mã chuỗi từ bảng mã nguồn về Unicode NFC. */
const char *vnt_charset_decode(VntController *c, const char *charset, const char *text);

/** Mã hoá chuỗi Unicode sang mảng byte thô cho bảng mã một byte. */
const uint8_t *vnt_charset_encode_bytes(VntController *c, const char *charset, const char *text,
                                        size_t *out_len);

/** Giải mã mảng byte thô từ bảng mã một byte về chuỗi Unicode NFC. */
const char *vnt_charset_decode_bytes(VntController *c, const char *charset, const uint8_t *data,
                                     size_t len);

/** Giải phóng chuỗi C do `vnt_settings_load_json` cấp phát. */
void vnt_free_string(char *s);

/** Trả về chuỗi JSON mô tả các khả năng giao diện của hệ điều hành hiện tại (`PlatformCapabilities`). */
const char *vnt_settings_capabilities_json(void);

/** Đọc cấu hình từ `config_path` (hoặc đường dẫn mặc định nếu NULL) dưới dạng JSON; caller tự giải phóng bằng `vnt_free_string`. */
char *vnt_settings_load_json(const char *config_path);

/** Kiểm tra hợp lệ bản vá JSON và lưu nguyên tử xuống `config_path` (hoặc đường dẫn mặc định nếu NULL). */
bool vnt_settings_validate_and_save_json(const char *config_path, const char *json);

/** Đảm bảo tệp `macros.txt` tương ứng với `config_path` tồn tại trên đĩa. */
bool vnt_macros_ensure_file(const char *config_path);

/** Trả về số lượng quy tắc gõ tắt đang có trong `macros.txt` (`-1` nếu lỗi). */
int32_t vnt_macros_entry_count(const char *config_path);

#ifdef __cplusplus
}
#endif

#endif /* VNTYPE_H */
