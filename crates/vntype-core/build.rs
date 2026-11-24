use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn decode_tsv_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('u') => {
                assert_eq!(chars.next(), Some('{'), "kỳ vọng '{{' sau \\u trong {raw:?}");
                let mut hex = String::new();
                for h in chars.by_ref() {
                    if h == '}' {
                        break;
                    }
                    hex.push(h);
                }
                let cp = u32::from_str_radix(&hex, 16)
                    .unwrap_or_else(|e| panic!("mã hex không hợp lệ {hex:?}: {e}"));
                let decoded =
                    char::from_u32(cp).unwrap_or_else(|| panic!("Unicode scalar không hợp lệ U+{cp:04X}"));
                out.push(decoded);
            }
            Some(other) => panic!("escape không hỗ trợ \\{other} trong {raw:?}"),
            None => panic!("dấu \\ ở cuối chuỗi {raw:?}"),
        }
    }
    out
}

fn find_charsets_dir(manifest_dir: &Path) -> PathBuf {
    let workspace_data = manifest_dir.join("../../data/charsets");
    if workspace_data.is_dir() {
        return workspace_data;
    }
    let local_data = manifest_dir.join("data/charsets");
    if local_data.is_dir() {
        return local_data;
    }
    panic!("Không tìm thấy thư mục bảng mã tại {} hoặc {}", workspace_data.display(), local_data.display());
}

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("thiếu OUT_DIR"));
    let out_file = out_dir.join("charset_tables_gen.rs");

    if env::var_os("CARGO_FEATURE_CHARSET").is_none() {
        fs::write(&out_file, "").expect("ghi file rỗng charset_tables_gen.rs");
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("thiếu CARGO_MANIFEST_DIR"));
    let charsets_dir = find_charsets_dir(&manifest_dir);
    println!("cargo:rerun-if-changed={}", charsets_dir.display());

    let index_path = charsets_dir.join("index.tsv");
    println!("cargo:rerun-if-changed={}", index_path.display());
    let index_text = fs::read_to_string(&index_path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", index_path.display()));

    let mut code = String::new();
    let mut specs = Vec::new();

    for raw_line in index_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, display_name) =
            line.split_once('\t').unwrap_or_else(|| panic!("dòng không hợp lệ trong index.tsv: {line:?}"));
        let const_name = format!("MAP_{}", key.to_ascii_uppercase());
        let table_path = charsets_dir.join(format!("{key}.tsv"));
        println!("cargo:rerun-if-changed={}", table_path.display());

        let table_text = fs::read_to_string(&table_path)
            .unwrap_or_else(|e| panic!("không đọc được {}: {e}", table_path.display()));

        code.push_str(&format!("const {const_name}: &[(char, &str)] = &[\n"));
        for entry_line in table_text.lines() {
            let trimmed = entry_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let (ch_str, val_raw) = trimmed
                .split_once('\t')
                .unwrap_or_else(|| panic!("dòng không hợp lệ trong {}: {trimmed:?}", table_path.display()));
            let mut chars = ch_str.chars();
            let ch = chars.next().expect("thiếu ký tự Unicode");
            assert!(chars.next().is_none(), "cột đầu tiên phải là đúng 1 ký tự Unicode: {ch_str:?}");
            let val = decode_tsv_value(val_raw);
            code.push_str(&format!("    ('{}', \"{}\"),\n", ch.escape_default(), val.escape_default()));
        }
        code.push_str("];\n\n");

        specs.push((key.to_string(), display_name.to_string(), const_name));
    }

    code.push_str(
        "/// Danh sách toàn bộ các bảng mã truyền thống được sinh lúc biên dịch từ `data/charsets/`.\n",
    );
    code.push_str("pub const STATIC_CHARSET_SPECS: &[StaticCharsetSpec] = &[\n");
    for (key, display_name, const_name) in specs {
        code.push_str(&format!(
            "    StaticCharsetSpec {{ key: \"{}\", display_name: \"{}\", mappings: {} }},\n",
            key.escape_default(),
            display_name.escape_default(),
            const_name
        ));
    }
    code.push_str("];\n");

    let mut file = fs::File::create(&out_file).expect("tạo charset_tables_gen.rs");
    file.write_all(code.as_bytes()).expect("ghi charset_tables_gen.rs");
}
