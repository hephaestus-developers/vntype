//! Lược đồ dữ liệu cấu hình [`Config`], hồ sơ ứng dụng [`AppProfile`] và hàm tra cứu ứng dụng.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use vntype_syllable::ToneStyle;

/// Phiên bản lược đồ cấu hình hiện hành.
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CONFIG_SCHEMA_VERSION
}

/// Chế độ xuất chữ từ bộ gõ sang ứng dụng đích.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    /// Hiển thị vùng văn bản gạch chân tạm thời (TSF, IMK, Fcitx5, IBus, Wayland).
    Preedit,
    /// Chốt trực tiếp và dùng lệnh xoá lùi khi cần sửa dấu.
    Replace,
}

/// Dạng tổ hợp Unicode tại đầu ra của bộ gõ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum UnicodeForm {
    /// Dạng dựng sẵn (Normalization Form C).
    #[default]
    Nfc,
    /// Dạng tổ hợp (Normalization Form D).
    Nfd,
}

/// Hồ sơ ghi đè hành vi và thông số thời gian trễ cho từng ứng dụng cụ thể.
///
/// Các trường đều ở dạng [`Option`] để phân biệt rõ giá trị chưa cấu hình với giá trị ghi đè tường minh
/// khi trộn hồ sơ mặc định (built-in) với tuỳ chỉnh của người dùng.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppProfile {
    /// Ghi đè chế độ xuất chữ (`Preedit` hoặc `Replace`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_mode: Option<OutputMode>,
    /// Độ trễ cơ sở (mili-giây) giữa các thao tác xoá lùi.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backspace_delay_ms: Option<u32>,
    /// Độ trễ cộng thêm (mili-giây) trên mỗi ký tự bị xoá lùi.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_per_char_ms: Option<u32>,
    /// Cờ yêu cầu chờ ứng dụng xử lý xong thao tác xoá trước khi chèn văn bản mới.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_ack: Option<bool>,
    /// Ghi đè cấu hình cho phép nạp lại từ trước con trỏ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_previous_word: Option<bool>,
    /// Ghi đè cấu hình khôi phục từ sau khi xoá dấu cách.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_after_space: Option<bool>,
    /// Ép trạng thái bật hoặc tắt tiếng Việt khi chuyển tiêu điểm vào ứng dụng này.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vietnamese: Option<bool>,
    /// Ghi đè kiểu gõ riêng cho ứng dụng.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_method: Option<String>,
    /// Ghi đè quy tắc đặt dấu thanh riêng cho ứng dụng.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone_style: Option<ToneStyle>,
    /// Ghi đè cấu hình kiểm tra chính tả riêng cho ứng dụng.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spell_check: Option<bool>,
    /// Gợi ý bảng mã đầu ra cho ứng dụng.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
}

impl AppProfile {
    /// Trộn một hồ sơ ghi đè `over` lên trên `self`; mọi trường `Some` của `over` sẽ thắng.
    pub fn merge(&self, over: &AppProfile) -> AppProfile {
        AppProfile {
            output_mode: over.output_mode.or(self.output_mode),
            backspace_delay_ms: over.backspace_delay_ms.or(self.backspace_delay_ms),
            delay_per_char_ms: over.delay_per_char_ms.or(self.delay_per_char_ms),
            wait_ack: over.wait_ack.or(self.wait_ack),
            edit_previous_word: over.edit_previous_word.or(self.edit_previous_word),
            restore_after_space: over.restore_after_space.or(self.restore_after_space),
            vietnamese: over.vietnamese.or(self.vietnamese),
            input_method: over.input_method.clone().or_else(|| self.input_method.clone()),
            tone_style: over.tone_style.or(self.tone_style),
            spell_check: over.spell_check.or(self.spell_check),
            charset: over.charset.clone().or_else(|| self.charset.clone()),
        }
    }
}

const APP_PROFILES_JSON: &str = include_str!("../../../data/profiles/app_profiles.json");

#[derive(Debug, Deserialize)]
struct AppProfileData {
    aliases: HashMap<String, String>,
    profiles: HashMap<String, AppProfile>,
}

fn app_profile_data() -> &'static AppProfileData {
    static DATA: OnceLock<AppProfileData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(APP_PROFILES_JSON).expect("app_profiles.json không hợp lệ"))
}

/// Trả về bản đồ ánh xạ bí danh ứng dụng tĩnh (ví dụ: `"google-chrome-stable"` -> `"chrome"`).
pub fn app_aliases() -> &'static HashMap<String, String> {
    &app_profile_data().aliases
}

/// Trả về danh sách hồ sơ mặc định (built-in) của các ứng dụng phổ biến.
pub fn default_app_profiles() -> &'static HashMap<String, AppProfile> {
    &app_profile_data().profiles
}

/// Cấu trúc cấu hình tổng thể của VNType.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Phương pháp gõ: `"telex"`, `"simple_telex"`, `"vni"`, hoặc `"viqr"`.
    pub input_method: String,
    /// Quy tắc đặt dấu thanh cũ hoặc mới.
    pub tone_style: ToneStyle,
    /// Bật kiểm tra chính tả tiếng Việt.
    pub spell_check: bool,
    /// Cho phép `w` đứng riêng biến thành `ư`.
    pub w_as_u: bool,
    /// Cho phép phím `[` và `]` biến thành `ơ` và `ư`.
    pub bracket_keys: bool,
    /// Bật gõ tắt phụ âm đầu.
    pub quick_telex: bool,
    /// Cho phép phụ âm cuối `k` cho tên riêng.
    pub allow_coda_k: bool,
    /// Bật tính năng gõ tắt (macro).
    pub macro_enabled: bool,
    /// Cho phép chữ số nằm trong từ gõ tắt.
    pub macro_digits: bool,
    /// Cho phép bấm Backspace để hoàn tác từ vừa bung gõ tắt.
    pub macro_undo: bool,
    /// Vẫn cho phép bung gõ tắt khi đang tắt chế độ tiếng Việt.
    pub macro_in_off_mode: bool,
    /// Bảng ánh xạ từ viết tắt sang chuỗi mở rộng.
    pub macros: HashMap<String, String>,
    /// Tự động viết hoa chữ cái đầu câu.
    pub auto_capitalize: bool,
    /// Danh sách định danh ứng dụng bị loại trừ (luôn tắt tiếng Việt).
    pub excluded_apps: Vec<String>,
    /// Trạng thái bật/tắt tiếng Việt đã lưu riêng cho từng ứng dụng.
    pub app_modes: HashMap<String, bool>,
    /// Trạng thái bật tiếng Việt mặc định cho ứng dụng chưa có trong `app_modes`.
    pub default_vietnamese: bool,
    /// Cho phép nạp lại từ vừa gõ trước con trỏ để sửa dấu.
    pub edit_previous_word: bool,
    /// Các hồ sơ ứng dụng do người dùng tự ghi đè (không chứa dữ liệu built-in khi lưu tệp).
    pub app_profiles: HashMap<String, AppProfile>,
    /// Cho phép khôi phục trạng thái từ cũ sau khi bấm Backspace xoá dấu cách.
    pub restore_after_space: bool,
    /// Giữ nguyên từ có 2 chữ cái đầu viết hoa liên tiếp.
    pub smart_caps: bool,
    /// Dạng chuẩn hoá Unicode tại đầu ra.
    pub unicode_form: UnicodeForm,
    /// Phiên bản lược đồ cấu hình.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            input_method: "telex".into(),
            tone_style: ToneStyle::New,
            spell_check: true,
            w_as_u: false,
            bracket_keys: false,
            quick_telex: false,
            allow_coda_k: false,
            macro_enabled: true,
            macro_digits: false,
            macro_undo: true,
            macro_in_off_mode: false,
            macros: HashMap::new(),
            auto_capitalize: false,
            excluded_apps: Vec::new(),
            app_modes: HashMap::new(),
            default_vietnamese: true,
            edit_previous_word: true,
            app_profiles: HashMap::new(),
            restore_after_space: true,
            smart_caps: false,
            unicode_form: UnicodeForm::Nfc,
            schema_version: CONFIG_SCHEMA_VERSION,
        }
    }
}

impl Config {
    /// Trộn một mảnh JSON cấu hình (partial JSON patch) lên trên cấu hình hiện tại.
    ///
    /// Trả về `None` nếu chuỗi JSON đầu vào không hợp lệ.
    ///
    /// # Examples
    ///
    /// ```
    /// use vntype_config::{Config, ToneStyle};
    ///
    /// let patched = Config::default().apply_json_patch(r#"{"tone_style": "old"}"#).unwrap();
    /// assert_eq!(patched.tone_style, ToneStyle::Old);
    /// ```
    pub fn apply_json_patch(&self, patch: &str) -> Option<Config> {
        let mut base = match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(m)) => m,
            _ => return None,
        };
        let patch: HashMap<String, serde_json::Value> = serde_json::from_str(patch).ok()?;
        for (k, v) in patch {
            base.insert(k, v);
        }
        serde_json::from_value(serde_json::Value::Object(base)).ok()
    }
}

/// Chuẩn hoá định danh ứng dụng: bỏ đường dẫn thư mục, bỏ hậu tố `.exe`/`.app`/`.desktop`,
/// lấy thành phần cuối của Bundle ID và ánh xạ qua bảng bí danh chuẩn.
///
/// # Examples
///
/// ```
/// use vntype_config::normalize_app_id;
///
/// assert_eq!(normalize_app_id(Some("com.google.Chrome")), "chrome");
/// assert_eq!(normalize_app_id(Some("/usr/bin/google-chrome-stable.desktop")), "chrome");
/// assert_eq!(normalize_app_id(None), "");
/// ```
pub fn normalize_app_id(app_id: Option<&str>) -> String {
    let Some(app) = app_id else { return String::new() };
    let mut key = app.trim().to_lowercase().replace('\\', "/");
    key = key.rsplit('/').next().unwrap_or("").to_string();
    for suffix in [".exe", ".app", ".desktop"] {
        if let Some(stripped) = key.strip_suffix(suffix) {
            key = stripped.to_string();
            break;
        }
    }
    if let Some((head, _)) = key.split_once(':') {
        key = head.to_string();
    }
    let head = key.split('.').next().unwrap_or("");
    if matches!(head, "com" | "org" | "io" | "net" | "app") {
        key = key.rsplit('.').next().unwrap_or("").to_string();
    }
    app_aliases().get(key.as_str()).cloned().unwrap_or(key)
}

/// Tra cứu [`AppProfile`] hợp nhất cho `app_id`: lấy hồ sơ built-in làm nền và ghi đè bằng
/// tuỳ chỉnh của người dùng trong `config.app_profiles` nếu có.
///
/// # Examples
///
/// ```
/// use vntype_config::{resolve_profile, Config, OutputMode};
///
/// let cfg = Config::default();
/// let profile = resolve_profile(&cfg, Some("google-chrome-stable"));
/// assert_eq!(profile.output_mode, Some(OutputMode::Preedit));
/// ```
pub fn resolve_profile(config: &Config, app_id: Option<&str>) -> AppProfile {
    let key = normalize_app_id(app_id);
    if key.is_empty() {
        return AppProfile::default();
    }
    let built_in = default_app_profiles().get(&key);
    let user = config.app_profiles.get(&key);
    match (built_in, user) {
        (Some(b), Some(u)) => b.merge(u),
        (Some(b), None) => b.clone(),
        (None, Some(u)) => u.clone(),
        (None, None) => AppProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_app_id_variants() {
        assert_eq!(normalize_app_id(None), "");
        assert_eq!(normalize_app_id(Some("C:\\Program Files\\Code.exe")), "vscode");
        assert_eq!(normalize_app_id(Some("org.wezfurlong.wezterm")), "wezterm");
        assert_eq!(normalize_app_id(Some("io.alacritty:1234")), "alacritty");
        assert_eq!(normalize_app_id(Some("custom-app")), "custom-app");
    }

    #[test]
    fn resolve_profile_merges_builtin_and_user_override() {
        let mut cfg = Config::default();
        assert!(cfg.app_profiles.is_empty());

        let base = resolve_profile(&cfg, Some("vscode"));
        assert_eq!(base.output_mode, Some(OutputMode::Preedit));
        assert_eq!(base.edit_previous_word, Some(false));

        cfg.app_profiles.insert(
            "vscode".into(),
            AppProfile { vietnamese: Some(false), edit_previous_word: Some(true), ..Default::default() },
        );
        let merged = resolve_profile(&cfg, Some("com.microsoft.VSCode"));
        // Giữ nguyên output_mode từ built-in, ghi đè vietnamese và edit_previous_word từ user
        assert_eq!(merged.output_mode, Some(OutputMode::Preedit));
        assert_eq!(merged.vietnamese, Some(false));
        assert_eq!(merged.edit_previous_word, Some(true));
    }

    #[test]
    fn default_config_serializes_without_builtin_profiles() {
        let json = serde_json::to_string(&Config::default()).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["app_profiles"], serde_json::json!({}));
    }

    #[test]
    fn apply_json_patch_valid_and_invalid() {
        let cfg = Config::default();
        let patched = cfg.apply_json_patch(r#"{"input_method": "vni", "spell_check": false}"#).unwrap();
        assert_eq!(patched.input_method, "vni");
        assert!(!patched.spell_check);
        assert!(cfg.apply_json_patch("{invalid").is_none());
    }
}
