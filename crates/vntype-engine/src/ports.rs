//! Định nghĩa các cổng ra (Driven Ports) của kiến trúc lục giác và các bản cài đặt sẵn.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub use vntype_config::{
    AppProfile, CONFIG_SCHEMA_VERSION, Config, OutputMode, SwitchKey, UnicodeForm, app_aliases,
    default_app_profiles, normalize_app_id, resolve_profile, save_config_atomic,
};

/// Cổng đọc và lưu cấu hình người dùng.
pub trait ConfigStore {
    /// Tải cấu hình hiện tại.
    fn load(&self) -> Config;
    /// Lưu cấu hình đã cập nhật (ví dụ khi người dùng bật/tắt tiếng Việt cho một ứng dụng).
    fn save(&self, config: &Config);
}

/// Cổng truy vấn thông tin ứng dụng đang nhận tiêu điểm bàn phím.
pub trait AppContext {
    /// Trả về định danh ứng dụng hiện tại (tên tiến trình, Bundle ID hoặc `WM_CLASS`).
    fn current_app(&self) -> Option<String>;
}

/// Cổng đọc văn bản xung quanh con trỏ soạn thảo của ứng dụng đích.
pub trait SurroundingText {
    /// Trả về tối đa `n` ký tự (Unicode scalar) nằm ngay trước vị trí con trỏ hiện tại.
    fn before_cursor(&self, n: usize) -> Option<String>;
}

/// Cổng tra cứu từ điển tiếng Anh và tiếng Việt để phục vụ khôi phục từ tự động.
pub trait Dictionary {
    /// Kiểm tra một chuỗi phím thô chữ thường có phải là từ tiếng Anh thông dụng hay không.
    fn is_english(&self, word: &str) -> bool;
    /// Kiểm tra một chuỗi chữ thường NFC có phải là âm tiết tiếng Việt thông dụng hay không.
    fn is_vietnamese(&self, word: &str) -> bool;
}

/// Tập hợp 4 cổng ra được tiêm vào [`crate::session::ImeSession`].
pub struct Ports {
    /// Cổng quản lý cấu hình.
    pub config: Box<dyn ConfigStore>,
    /// Cổng truy vấn ngữ cảnh ứng dụng.
    pub app: Box<dyn AppContext>,
    /// Cổng truy vấn văn bản trước con trỏ.
    pub surrounding: Box<dyn SurroundingText>,
    /// Cổng tra cứu từ điển.
    pub dictionary: Box<dyn Dictionary>,
}

/// Kho cấu hình lưu trực tiếp trong bộ nhớ RAM (dùng cho kiểm thử hoặc nhúng).
#[derive(Default)]
pub struct MemoryConfigStore {
    c: RefCell<Config>,
}

impl MemoryConfigStore {
    /// Tạo một kho cấu hình trong bộ nhớ với giá trị khởi tạo `config`.
    pub fn new(config: Config) -> Self {
        MemoryConfigStore { c: RefCell::new(config) }
    }
}

impl ConfigStore for MemoryConfigStore {
    fn load(&self) -> Config {
        self.c.borrow().clone()
    }
    fn save(&self, config: &Config) {
        *self.c.borrow_mut() = config.clone();
    }
}

/// Bộ bọc tham chiếu đếm (`Rc`) quanh [`MemoryConfigStore`] để chia sẻ giữa phiên gõ và trình kiểm thử.
#[derive(Clone, Default)]
pub struct SharedConfigStore(pub Rc<MemoryConfigStore>);

impl ConfigStore for SharedConfigStore {
    fn load(&self) -> Config {
        self.0.load()
    }
    fn save(&self, config: &Config) {
        self.0.save(config)
    }
}

/// Kho cấu hình đọc và ghi nguyên tử trên tệp JSON của hệ thống tệp tin.
pub struct JsonFileConfigStore {
    /// Đường dẫn tới tệp cấu hình JSON.
    pub path: PathBuf,
}

impl JsonFileConfigStore {
    /// Tạo kho cấu hình gắn với đường dẫn tệp `path`.
    pub fn new(path: impl AsRef<Path>) -> Self {
        JsonFileConfigStore { path: path.as_ref().to_path_buf() }
    }
}

impl ConfigStore for JsonFileConfigStore {
    fn load(&self) -> Config {
        vntype_config::load_config_or_default(&self.path)
    }
    fn save(&self, config: &Config) {
        let _ = save_config_atomic(&self.path, config);
    }
}

/// Ngữ cảnh ứng dụng cố định (dùng cho các nền tảng không báo `app_id` hoặc khi kiểm thử).
pub struct StaticAppContext(pub Option<String>);

impl StaticAppContext {
    /// Tạo ngữ cảnh ứng dụng cố định với tên `app`.
    pub fn new(app: &str) -> Self {
        StaticAppContext(Some(app.to_string()))
    }
}

impl AppContext for StaticAppContext {
    fn current_app(&self) -> Option<String> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_and_shared_config_stores() {
        let shared = SharedConfigStore::default();
        let mut cfg = shared.load();
        cfg.input_method = "vni".into();
        shared.save(&cfg);
        assert_eq!(shared.load().input_method, "vni");

        let app = StaticAppContext::new("chrome");
        assert_eq!(app.current_app().as_deref(), Some("chrome"));
    }
}
