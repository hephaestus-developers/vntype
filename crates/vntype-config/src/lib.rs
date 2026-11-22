//! Quản lý lược đồ cấu hình, hồ sơ ứng dụng (`AppProfile`), chuẩn hoá định danh ứng dụng
//! và lưu tệp nguyên tử (`vntype-config`).

pub mod atomic_save;
pub mod loader;
pub mod paths;
pub mod schema;

pub use atomic_save::{ConfigError, save_config_atomic};
pub use loader::{fingerprint_bytes, load_config_file, load_config_or_default};
pub use paths::{default_config_dir, default_config_path, default_user_words_path};
pub use schema::{
    AppProfile, CONFIG_SCHEMA_VERSION, Config, OutputMode, UnicodeForm, app_aliases, default_app_profiles,
    normalize_app_id, resolve_profile,
};
pub use vntype_syllable::ToneStyle;
