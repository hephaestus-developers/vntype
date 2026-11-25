# Chính sách bảo mật (Security Policy)

## Các phiên bản được hỗ trợ

Chúng tôi cung cấp bản vá bảo mật cho phiên bản phát hành mới nhất trên nhánh `main` (theo chu kỳ `{year}.{month}.{number}`).

| Phiên bản | Hỗ trợ |
| --------- | ------ |
| `2026.11.x` | Có |
| `< 2026.11.0` | Không |

## Báo cáo lỗ hổng bảo mật

**VNType** là bộ gõ chạy nền xử lý sự kiện bàn phím ở tầng hệ điều hành (IBus/Fcitx5 trên Linux, `WH_KEYBOARD_LL`/TSF trên Windows, InputMethodKit trên macOS). Do đó, mọi vấn đề liên quan đến:

- Rò rỉ dữ liệu gõ phím (ví dụ ở ô nhập mật khẩu `FieldKind::Password`),
- Leo thang đặc quyền hoặc ghi tệp cấu hình/log không an toàn,
- Lỗ hổng phân tích dữ liệu / crash tiến trình hệ thống,

vui lòng **không** mở Issue công khai. Thay vào đó, hãy báo cáo qua một trong hai kênh sau:

1. **GitHub Private Vulnerability Reporting**: Truy cập tab **Security → Advisories → Report a vulnerability** trên kho lưu trữ [hephaestus-developers/vntype](https://github.com/hephaestus-developers/vntype).
2. **Email**: Gửi chi tiết tới `haiphamngoc.tech@gmail.com` với tiêu đề `[VNType Security] ...`.

Vui lòng đính kèm mô tả lỗ hổng, phiên bản bị ảnh hưởng, môi trường hệ điều hành và các bước tái hiện (nếu có). Chúng tôi sẽ phản hồi xác nhận trong vòng 72 giờ.
