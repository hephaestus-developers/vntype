//! Tầng ứng dụng (Application Layer) của kiến trúc lục giác VNType (`vntype-engine`).
//!
//! Crate này cung cấp:
//! - [`ImeSession`]: Máy trạng thái phiên gõ cho từng ô nhập liệu.
//! - [`ports`]: Giao diện các cổng ra (`ConfigStore`, `AppContext`, `SurroundingText`, `Dictionary`).
//! - [`dictionary`]: Từ điển dùng chung trong bộ nhớ với cơ chế sao chép khi ghi (Copy-on-Write).
//! - [`macro_table`]: Bộ xử lý bảng gõ tắt đơn từ và cụm từ, giữ nguyên quy tắc viết hoa.

pub mod dictionary;
pub mod macro_table;
pub mod ports;
pub mod session;

pub use dictionary::{WordListDictionary, shared_dictionary};
pub use macro_table::{expand_single_macro, match_phrase_macro, py_isupper};
pub use ports::{
    AppContext, AppProfile, CONFIG_SCHEMA_VERSION, Config, ConfigStore, Dictionary, JsonFileConfigStore,
    MemoryConfigStore, OutputMode, Ports, SharedConfigStore, StaticAppContext, SurroundingText, SwitchKey,
    UnicodeForm, app_aliases, default_app_profiles, normalize_app_id, resolve_profile,
};
pub use session::{Action, FieldKind, ImeSession, Key, KeyEvent};
