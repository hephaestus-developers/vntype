//! Lõi quản lý cấu hình, kiểm tra hợp lệ và khả năng giao diện theo hệ điều hành (`vntype-settings-core`).

pub mod capabilities;
pub mod view_model;

pub use capabilities::PlatformCapabilities;
pub use view_model::{SettingsViewModel, macros_path_for_config, validate_and_save_json};
