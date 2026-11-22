//! Lõi biến đổi chuỗi phím thô sang chữ tiếng Việt hiển thị (`vntype-core`).
//!
//! Crate này hoàn toàn thuần hàm, đồng bộ và không chứa bất kỳ thao tác vào/ra nào.
//! Nó bao gồm:
//! - [`rules`]: Định nghĩa bảng luật kiểu gõ Telex, Simple Telex, VNI và VIQR.
//! - [`composer`]: Máy trạng thái biến đổi tăng dần từng phím gõ (`Composer`).
//! - [`mod@rebuild`]: Phép nghịch đảo khôi phục chuỗi phím thô từ chữ tiếng Việt trên màn hình.
//! - [`mod@diff`]: Tính toán thao tác xoá lùi và chèn tối thiểu giữa hai chuỗi hiển thị.
//! - `charset`: Bộ chuyển đổi giữa 16 bảng mã tiếng Việt (Unicode NFC/NFD, TCVN3, VNI-Windows, VIQR...).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "charset")]
pub mod charset;
#[cfg(feature = "charset")]
mod charset_tables;
pub mod composer;
pub mod diff;
pub mod rebuild;
pub mod rules;

#[cfg(feature = "charset")]
pub use charset::{
    CHARSETS, change_case, charset_names, convert as convert_charset, decode as decode_charset,
    decode_bytes, detect as detect_charset, display_name as charset_display_name, encode as encode_charset,
    encode_bytes, is_single_byte, normalize as normalize_charset, remove_diacritics,
};
pub use composer::{
    Composer, Letter, MAX_LETTERS, MAX_RAW, TransformOptions, TransformResult, WordState, transform,
    transform_str,
};
pub use diff::{Edit, diff};
pub use rebuild::{rebuild, to_raw};
pub use rules::{InputMethodSpec, MarkRule, SIMPLE_TELEX, TELEX, VIQR, VNI, quick_telex};
pub use vntype_syllable::{Mark, Status, ToneStyle};
