#!/usr/bin/env bash
# Tập lệnh biên dịch addon Fcitx5 (liên kết tĩnh libvntype_ffi.a), tạo symlink compile_commands.json và chạy CTest.
# Cách dùng:
#   ./platforms/linux-fcitx5/build.sh             # Biên dịch đầy đủ addon Fcitx5 + test
#   ./platforms/linux-fcitx5/build.sh --test-only # Chỉ biên dịch và chạy test C ABI (không cần header Fcitx5)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

TEST_ONLY=OFF
if [ "${1:-}" = "--test-only" ]; then
    TEST_ONLY=ON
fi

CARGO_FLAGS=()
if [ "${VNTYPE_OFFLINE:-0}" = "1" ] || ! timeout 1 bash -c "</dev/tcp/index.crates.io/443" 2>/dev/null; then
    CARGO_FLAGS+=("--offline")
fi

echo "==> [1/3] Biên dịch thư viện tĩnh Rust libvntype_ffi.a"
(cd "$ROOT" && cargo build --release -p vntype-ffi "${CARGO_FLAGS[@]}")

echo "==> [2/3] Cấu hình và biên dịch CMake cho vntype-fcitx5"
cmake -S "$HERE" -B "$HERE/build" \
    -DVNTYPE_STATIC_LIB="$ROOT/target/release/libvntype_ffi.a" \
    -DVNTYPE_FCITX5_TEST_ONLY="$TEST_ONLY" \
    -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_TESTING=ON
cmake --build "$HERE/build" -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"

if [ -f "$HERE/build/compile_commands.json" ]; then
    ln -sf "$HERE/build/compile_commands.json" "$HERE/compile_commands.json"
fi

echo "==> [3/3] Chạy bộ kiểm thử CTest của addon Fcitx5"
ctest --test-dir "$HERE/build" --output-on-failure
