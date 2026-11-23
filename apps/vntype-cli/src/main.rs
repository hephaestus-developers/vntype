//! Công cụ dòng lệnh `vntype` phục vụ kiểm thử nhanh, chuyển đổi bảng mã và chẩn đoán cấu hình.

use vntype_adapter::{AdapterController, Effect};
use vntype_config::{ToneStyle, default_app_profiles, default_config_path, load_config_or_default};
use vntype_core::convert_charset;
use vntype_engine::{Config, KeyEvent, OutputMode};

/// Mô phỏng việc gõ một chuỗi phím `raw_input` qua [`AdapterController`] và trả về văn bản cuối cùng.
pub fn simulate_typing(raw_input: &str, config: Config) -> String {
    let mut ctrl = AdapterController::new(config, OutputMode::Preedit);
    let mut committed = String::new();
    let mut preedit = String::new();

    for (i, ch) in raw_input.chars().enumerate() {
        let effects = ctrl.process_key(KeyEvent::ch(ch), i as u32 + 1);
        apply_to_buffer(&effects, &mut committed, &mut preedit, Some(ch));
    }
    let flush_effects = ctrl.flush();
    apply_to_buffer(&flush_effects, &mut committed, &mut preedit, None);
    committed.push_str(&preedit);
    committed
}

fn apply_to_buffer(
    effects: &[Effect],
    committed: &mut String,
    preedit: &mut String,
    current_char: Option<char>,
) {
    for eff in effects {
        match eff {
            Effect::SetPreedit { text, .. } => {
                *preedit = text.clone();
            }
            Effect::ClearPreedit => {
                preedit.clear();
            }
            Effect::CommitText { text } => {
                committed.push_str(text);
            }
            Effect::ReplaceText { delete_chars, insert, .. } => {
                for _ in 0..*delete_chars {
                    let _ = committed.pop();
                }
                committed.push_str(insert);
            }
            Effect::WaitMs { .. } => {}
            Effect::ForwardKey { .. } => {
                if let Some(c) = current_char {
                    committed.push(c);
                }
            }
        }
    }
}

fn run_demo() {
    let samples = [
        ("telex", ToneStyle::New, "Tieengs Vieejt ddaawrng caaps"),
        ("telex", ToneStyle::New, "DDawks Lawks Vieejt Nam"),
        ("vni", ToneStyle::New, "Vie65t Nam hoa2 binh2"),
    ];
    for (method, tone, input) in samples {
        let cfg = Config {
            input_method: method.into(),
            tone_style: tone,
            allow_coda_k: true,
            ..Config::default()
        };
        let out = simulate_typing(input, cfg);
        println!("[{method}] {input} -> {out}");
    }
}

fn print_help() {
    println!(
        "\
vntype {} — Công cụ dòng lệnh VNType

Cách dùng:
  vntype [TUỲ CHỌN]

Tuỳ chọn:
  --demo                 Chạy mẫu gõ tiếng Việt
  --doctor               Kiểm tra đường dẫn và thông số cấu hình hiện tại
  --text <CHUỖI>         Mô phỏng gõ chuỗi phím thô
  --method <KIỂU_GÕ>     Kiểu gõ: telex | simple_telex | vni | viqr (mặc định: telex)
  --tone <QUY_TẮC>       Quy tắc dấu thanh: new | old (mặc định: new)
  --to-charset <BẢNG_MÃ> Chuyển kết quả sang bảng mã đích (vd: viqr, tcvn3)
  -h, --help             Hiển thị trợ giúp
  -V, --version          Hiển thị phiên bản",
        env!("CARGO_PKG_VERSION")
    );
}

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next().unwrap_or_else(|| {
        eprintln!("Thiếu giá trị cho tham số {flag}");
        std::process::exit(2);
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut method = String::from("telex");
    let mut tone_style = ToneStyle::New;
    let mut text: Option<String> = None;
    let mut to_charset: Option<String> = None;
    let mut demo = false;
    let mut doctor = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                return;
            }
            "--version" | "-V" => {
                println!("vntype {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--demo" => demo = true,
            "--doctor" => doctor = true,
            "--method" => method = next_arg(&mut args, "--method"),
            "--tone" => {
                let t = next_arg(&mut args, "--tone");
                tone_style = if t == "old" { ToneStyle::Old } else { ToneStyle::New };
            }
            "--text" => text = Some(next_arg(&mut args, "--text")),
            "--to-charset" => to_charset = Some(next_arg(&mut args, "--to-charset")),
            other => {
                eprintln!("Tham số không hợp lệ: {other}");
                std::process::exit(2);
            }
        }
    }

    if doctor {
        let path = default_config_path();
        let cfg = load_config_or_default(&path);
        println!("Đường dẫn cấu hình: {}", path.display());
        println!("Kiểu gõ hiện tại: {}", cfg.input_method);
        println!(
            "Hồ sơ ứng dụng: {} mặc định, {} ghi đè",
            default_app_profiles().len(),
            cfg.app_profiles.len()
        );
        return;
    }

    if demo {
        run_demo();
        return;
    }

    if let Some(input) = text {
        let cfg = Config { input_method: method, tone_style, allow_coda_k: true, ..Config::default() };
        let typed = simulate_typing(&input, cfg);
        if let Some(cs) = to_charset {
            println!("{}", convert_charset(&typed, "unicode", &cs));
        } else {
            println!("{typed}");
        }
    } else {
        run_demo();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_simulation_produces_accurate_vietnamese() {
        let cfg = Config { allow_coda_k: true, ..Config::default() };
        assert_eq!(simulate_typing("Vieejt Nam", cfg.clone()), "Việt Nam");
        assert_eq!(simulate_typing("DDawks Lawks", cfg), "Đắk Lắk");

        let vni = Config { input_method: "vni".into(), ..Config::default() };
        assert_eq!(simulate_typing("Vie65t Nam", vni), "Việt Nam");
    }
}
