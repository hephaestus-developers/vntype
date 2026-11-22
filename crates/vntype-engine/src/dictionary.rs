//! Từ điển tiếng Việt và tiếng Anh dùng chung trong bộ nhớ với cơ chế Copy-on-Write (`Arc`).

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::ports::Dictionary;

const ENGLISH_COMMON: &str = include_str!("../../../data/dict/en_common.txt");
const VIETNAMESE_COMMON: &str = include_str!("../../../data/dict/vi_syllables.txt");

/// Bộ từ điển lưu trữ bằng bảng băm [`HashSet`] cho phép tra cứu trong thời gian hằng số $O(1)$.
#[derive(Clone, Debug)]
pub struct WordListDictionary {
    /// Tập hợp các từ tiếng Anh thông dụng ở dạng chữ thường.
    pub english: HashSet<String>,
    /// Tập hợp các âm tiết tiếng Việt ở dạng chữ thường chuẩn NFC.
    pub vietnamese: HashSet<String>,
}

impl WordListDictionary {
    /// Khởi tạo từ điển từ hai chuỗi văn bản chứa các từ phân cách bằng khoảng trắng.
    pub fn from_text(english: &str, vietnamese: &str) -> Self {
        WordListDictionary {
            english: english.split_whitespace().map(str::to_string).collect(),
            vietnamese: vietnamese.split_whitespace().map(str::to_string).collect(),
        }
    }

    /// Bổ sung một danh sách từ mới vào từ điển tiếng Việt (`vietnamese = true`) hoặc tiếng Anh (`false`).
    pub fn add_words<I, S>(&mut self, words: I, vietnamese: bool)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let target = if vietnamese { &mut self.vietnamese } else { &mut self.english };
        for word in words {
            let word = word.as_ref().trim().to_lowercase();
            if !word.is_empty() {
                target.insert(word);
            }
        }
    }

    /// Đọc và nạp thêm danh sách từ cá nhân của người dùng từ một tệp văn bản.
    pub fn load_user_file(&mut self, path: impl AsRef<Path>, vietnamese: bool) {
        if let Ok(text) = std::fs::read_to_string(path) {
            self.add_words(text.split_whitespace(), vietnamese);
        }
    }
}

impl Default for WordListDictionary {
    fn default() -> Self {
        WordListDictionary::from_text(ENGLISH_COMMON, VIETNAMESE_COMMON)
    }
}

impl Dictionary for WordListDictionary {
    fn is_english(&self, word: &str) -> bool {
        self.english.contains(word)
    }
    fn is_vietnamese(&self, word: &str) -> bool {
        self.vietnamese.contains(word)
    }
}

impl Dictionary for Arc<WordListDictionary> {
    fn is_english(&self, word: &str) -> bool {
        self.english.contains(word)
    }
    fn is_vietnamese(&self, word: &str) -> bool {
        self.vietnamese.contains(word)
    }
}

/// Trả về tham chiếu đếm nguyên tử (`Arc`) tới thực thể từ điển mặc định duy nhất trong toàn tiến trình.
///
/// Nhờ cơ chế này, dù trình quản lý bộ gõ mở hàng chục phiên cho các cửa sổ khác nhau,
/// dữ liệu từ điển chỉ được khởi tạo và chiếm bộ nhớ RAM đúng một lần.
///
/// # Examples
///
/// ```
/// use vntype_engine::{shared_dictionary, Dictionary};
///
/// let dict = shared_dictionary();
/// assert!(dict.is_english("hello"));
/// assert!(dict.is_vietnamese("việt"));
/// ```
pub fn shared_dictionary() -> Arc<WordListDictionary> {
    static DICT: OnceLock<Arc<WordListDictionary>> = OnceLock::new();
    DICT.get_or_init(|| Arc::new(WordListDictionary::default())).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_lookup_and_add_words() {
        let mut dict = WordListDictionary::from_text("hello", "chào");
        assert!(dict.is_english("hello"));
        assert!(!dict.is_english("world"));
        dict.add_words(["  World  ", ""], false);
        assert!(dict.is_english("world"));

        let shared_a = shared_dictionary();
        let shared_b = shared_dictionary();
        assert!(Arc::ptr_eq(&shared_a, &shared_b));
    }
}
