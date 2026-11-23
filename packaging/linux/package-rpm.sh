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
    -p vntype-settings \
    -p vntype-ibus \
    -p vntype-wayland \
    -p vntype-ffi \
    --manifest-path "$ROOT_DIR/Cargo.toml"

TOPDIR="$(mktemp -d)"
trap 'rm -rf "$TOPDIR"' EXIT
mkdir -p "$TOPDIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS} "$OUT_DIR"

FCITX_BUILD="$TOPDIR/fcitx5-build"
cmake -B "$FCITX_BUILD" -S "$ROOT_DIR/platforms/linux/fcitx5" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/usr
cmake --build "$FCITX_BUILD" -j"$(nproc)"

STAGE="$TOPDIR/SOURCES/stage"
install -Dm755 "$ROOT_DIR/target/release/vntype" "$STAGE/usr/bin/vntype"
install -Dm755 "$ROOT_DIR/target/release/vntype-settings" "$STAGE/usr/bin/vntype-settings"
install -Dm755 "$ROOT_DIR/target/release/vntype-ibus" "$STAGE/usr/libexec/vntype-ibus"
mkdir -p "$STAGE/usr/share/ibus/component"
sed 's|@LIBEXECDIR@|/usr/libexec|g' \
    "$ROOT_DIR/platforms/linux/ibus/data/vntype.xml.in" \
    > "$STAGE/usr/share/ibus/component/vntype.xml"
install -Dm755 "$ROOT_DIR/target/release/vntype-wayland" "$STAGE/usr/bin/vntype-wayland"
mkdir -p "$STAGE/usr/lib/systemd/user"
sed 's|%h/.local/bin|/usr/bin|g' \
    "$ROOT_DIR/platforms/linux/wayland/data/vntype-wayland.service" \
    > "$STAGE/usr/lib/systemd/user/vntype-wayland.service"
DESTDIR="$STAGE" cmake --install "$FCITX_BUILD"

cat > "$TOPDIR/SPECS/vntype.spec" <<EOF
Name:           vntype-common
Version:        $VERSION
Release:        1%{?dist}
Summary:        VNType Vietnamese input method CLI and settings UI
License:        MIT OR Apache-2.0

%description
VNType Vietnamese input method CLI and settings UI.

%package -n vntype-ibus
Summary:        VNType IBus input method engine
Requires:       vntype-common = %{version}-%{release}, ibus

%description -n vntype-ibus
VNType IBus input method engine.

%package -n vntype-wayland
Summary:        VNType Wayland virtual-keyboard input method
Requires:       vntype-common = %{version}-%{release}

%description -n vntype-wayland
VNType Wayland virtual-keyboard input method.

%package -n vntype-fcitx5
Summary:        VNType Fcitx5 input method addon
Requires:       vntype-common = %{version}-%{release}, fcitx5

%description -n vntype-fcitx5
VNType Fcitx5 input method addon.

%install
rm -rf %{buildroot}
cp -a "$STAGE"/* %{buildroot}/

%files
%{_bindir}/vntype
%{_bindir}/vntype-settings

%files -n vntype-ibus
%{_libexecdir}/vntype-ibus
%{_datadir}/ibus/component/vntype.xml

%files -n vntype-wayland
%{_bindir}/vntype-wayland
%{_userunitdir}/vntype-wayland.service

%files -n vntype-fcitx5
%{_libdir}/fcitx5/libvntype.so
%{_datadir}/fcitx5/addon/vntype.conf
%{_datadir}/fcitx5/inputmethod/vntype.conf
EOF

rpmbuild --define "_topdir $TOPDIR" --target "$ARCH" -bb "$TOPDIR/SPECS/vntype.spec"
find "$TOPDIR/RPMS" -name '*.rpm' -exec cp -t "$OUT_DIR" {} +

echo "Created .rpm packages in $OUT_DIR:"
ls -lh "$OUT_DIR"/*.rpm
