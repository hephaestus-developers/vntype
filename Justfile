# Liệt kê các lệnh có sẵn
default:
    @just --list

# Định dạng toàn bộ mã nguồn bằng rustfmt
fmt:
    cargo fmt --all

# Kiểm tra định dạng mã nguồn (không ghi đè)
fmt-check:
    cargo fmt --all -- --check

# Chạy clippy với toàn bộ cảnh báo ở mức lỗi
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Kiểm tra định dạng và lint toàn workspace
lint: fmt-check clippy

# Kiểm tra biên dịch mặc định và các cấu hình no_std / features
check:
    cargo check --workspace --all-targets
    cargo check -p vntype-syllable --no-default-features
    cargo check -p vntype-core --no-default-features
    cargo check -p vntype-core --no-default-features --features std

# Chạy toàn bộ unit test trong workspace
test *args:
    cargo test --workspace {{args}}

# Chạy bộ benchmark hiệu năng (Criterion)
bench *args:
    cargo bench -p vntype-syllable -p vntype-core -p vntype-engine -p vntype-adapter {{args}}

# Tạo tài liệu rustdoc và báo lỗi nếu có cảnh báo
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Biên dịch bản debug toàn workspace
build:
    cargo build --workspace

# Biên dịch bản release tối ưu toàn workspace
release:
    cargo build --workspace --release

# Chạy công cụ dòng lệnh vntype
cli *args:
    cargo run -p vntype-cli -- {{args}}

# Chạy ứng dụng cấu hình vntype-settings
settings *args:
    cargo run -p vntype-settings -- {{args}}

# Chạy đầy đủ quy trình kiểm tra như trên CI
ci: lint check test doc

# Dọn dẹp thư mục target
clean:
    cargo clean
