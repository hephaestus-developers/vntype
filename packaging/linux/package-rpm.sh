#!/usr/bin/env bash
set -euo pipefail
umask 022

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/dist/linux}"
OFFLINE="${VNTYPE_OFFLINE:-0}"

for cmd in cargo cmake rpmbuild; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Missing required tool: $cmd" >&2
        exit 1
    fi
done

VERSION="${VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)}"
ARCH="$(uname -m)"

CARGO_FLAGS=(--release)
[[ "$OFFLINE" == "1" ]] && CARGO_FLAGS+=(--offline)

echo "Building VNType $VERSION ($ARCH) release binaries..."
cargo build "${CARGO_FLAGS[@]}" \
    -p vntype-cli \
    -p vntype-ibus \
    -p vntype-ffi \
    --manifest-path "$ROOT_DIR/Cargo.toml"

TOPDIR="$(mktemp -d)"
trap 'rm -rf "$TOPDIR"' EXIT
mkdir -p "$TOPDIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS} "$OUT_DIR"

FCITX_BUILD="$TOPDIR/fcitx5-build"
RPM_LIBDIR="$(rpm --eval '%{_libdir}' | sed 's|^/||')"
cmake -B "$FCITX_BUILD" -S "$ROOT_DIR/platforms/linux/fcitx5" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/usr \
    -DCMAKE_INSTALL_LIBDIR="$RPM_LIBDIR"
cmake --build "$FCITX_BUILD" -j"$(nproc)"

STAGE="$TOPDIR/SOURCES/stage"
install -Dm755 "$ROOT_DIR/target/release/vntype-cli" "$STAGE/usr/bin/vntype-cli"
install -Dm755 "$ROOT_DIR/target/release/vntype-ibus" "$STAGE/usr/libexec/vntype-ibus"
mkdir -p "$STAGE/usr/share/ibus/component"
sed 's|@LIBEXECDIR@|/usr/libexec|g' \
    "$ROOT_DIR/platforms/linux/ibus/data/vntype.xml.in" \
    > "$STAGE/usr/share/ibus/component/vntype.xml"
DESTDIR="$STAGE" cmake --install "$FCITX_BUILD"

cat > "$TOPDIR/SPECS/vntype.spec" <<EOF
Name:           vntype
Version:        $VERSION
Release:        1%{?dist}
Summary:        VNType Vietnamese input method
License:        MIT

%description
VNType Vietnamese input method.

%package -n vntype-cli
Summary:        VNType Vietnamese input method CLI

%description -n vntype-cli
VNType Vietnamese input method CLI.

%package -n vntype-ibus
Summary:        VNType IBus input method engine
Requires:       ibus

%description -n vntype-ibus
VNType IBus input method engine.

%package -n vntype-fcitx5
Summary:        VNType Fcitx5 input method addon
Requires:       fcitx5

%description -n vntype-fcitx5
VNType Fcitx5 input method addon.

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a "$STAGE"/* %{buildroot}/

%files -n vntype-cli
%{_bindir}/vntype-cli

%files -n vntype-ibus
%{_libexecdir}/vntype-ibus
%{_datadir}/ibus/component/vntype.xml

%files -n vntype-fcitx5
%{_libdir}/fcitx5/libvntype.so
%{_datadir}/fcitx5/addon/vntype.conf
%{_datadir}/fcitx5/inputmethod/vntype.conf
EOF

rpmbuild --define "_topdir $TOPDIR" --target "$ARCH" -bb "$TOPDIR/SPECS/vntype.spec"
find "$TOPDIR/RPMS" -name '*.rpm' -exec cp -t "$OUT_DIR" {} +

echo "Created .rpm packages in $OUT_DIR:"
ls -lh "$OUT_DIR"/*.rpm
