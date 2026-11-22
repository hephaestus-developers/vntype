//! Mô hình âm vị học tiếng Việt thuần khiết, không phụ thuộc hệ điều hành hay I/O.
//!
//! Crate này cung cấp các thành phần cốt lõi để phân tích một âm tiết tiếng Việt:
//! - Tách chữ thành phụ âm đầu (`onset`), cụm nguyên âm (`cluster`) và phụ âm cuối (`coda`).
//! - Phân rã và tổ hợp ký tự tiếng Việt theo chuẩn Unicode NFC/NFD (`compose_char`, `decompose_char`).
//! - Kiểm tra chính tả ba trạng thái (`Valid`, `Partial`, `Invalid`) hỗ trợ gõ dấu tự do.
//! - Xác định vị trí đặt dấu thanh theo quy tắc cũ (`hòa`, `thúy`) hoặc mới (`hoà`, `thuý`).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod case;
pub mod phonology;
pub mod spellcheck;
pub mod tone;

pub use case::{lower, upper};
pub use phonology::{
    Decomposed, Mark, Syllable, VOWELS, compose_char, decompose_char, is_letter, is_vowel, parse,
    strip_tone,
};
pub use spellcheck::{Status, check, check_final, check_with, tone_allowed, tone_allowed_with};
pub use tone::{ToneStyle, place_tone, tone_index};
