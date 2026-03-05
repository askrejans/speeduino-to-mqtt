#!/usr/bin/env bash
# =============================================================================
# build_packages.sh – Cross-compile speeduino-to-mqtt and create DEB + RPM packages
#
# Usage:
#   ./scripts/build_packages.sh [options]
#
# Options:
#   --arch  x64|arm64|all   CPU architecture targets (default: all)
#   --type  deb|rpm|all     Package format (default: all)
#   --no-cross              Use local cargo instead of 'cross' for cross-compilation
#   --help                  Show this help
#
# Prerequisites:
#   • Rust toolchain (rustup)
#   • cross   (cargo install cross)  – or --no-cross with the right toolchains installed
#   • dpkg-deb  (Debian/Ubuntu: apt-get install dpkg-dev)
#   • rpmbuild  (Fedora/RHEL:   dnf install rpm-build)
#   • fpm       (optional, auto-detected – pip install fpm)
#
# The generated packages are placed in release/<version>/{deb,rpm}/.
# A systemd service file is bundled so `apt install` / `rpm -i` will install
# and enable the service automatically.
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
ARCH_TARGET="all"
PKG_TYPE="all"
USE_CROSS=true
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch)   ARCH_TARGET="$2"; shift 2 ;;
        --type)   PKG_TYPE="$2";    shift 2 ;;
        --no-cross) USE_CROSS=false; shift ;;
        --help)
            head -20 "$0" | grep "^#" | sed 's/^# \?//'
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
log()  { echo "[BUILD] $*"; }
warn() { echo "[WARN]  $*" >&2; }
die()  { echo "[ERROR] $*" >&2; exit 1; }

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is not installed. $2"
}

# ---------------------------------------------------------------------------
# Resolve package metadata
# ---------------------------------------------------------------------------
PKG_NAME="speeduino-to-mqtt"
PKG_VERSION="$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
PKG_DESCRIPTION="Speeduino ECU to MQTT bridge service"
PKG_MAINTAINER="askrejans <arvis.skrejans@gmail.com>"
PKG_LICENSE="MIT"
PKG_URL="https://github.com/askrejans/speeduino-to-mqtt"
SERVICE_FILE="$PROJECT_DIR/speeduino-to-mqtt.service"

log "Package: $PKG_NAME v$PKG_VERSION"

RELEASE_DIR="$PROJECT_DIR/release/$PKG_VERSION"
DEB_DIR="$RELEASE_DIR/deb"
RPM_DIR="$RELEASE_DIR/rpm"

# ---------------------------------------------------------------------------
# Select target triples
# ---------------------------------------------------------------------------
get_triple() {
    case "$1" in
        x64)   echo "x86_64-unknown-linux-gnu" ;;
        arm64) echo "aarch64-unknown-linux-gnu" ;;
        *)     die "Unknown arch '$1'" ;;
    esac
}

case "$ARCH_TARGET" in
    x64)   TARGETS=("x64") ;;
    arm64) TARGETS=("arm64") ;;
    all)   TARGETS=("x64" "arm64") ;;
    *)     die "Unknown arch '$ARCH_TARGET'. Use x64, arm64, or all." ;;
esac

# ---------------------------------------------------------------------------
# Build binaries
# ---------------------------------------------------------------------------
build_binary() {
    local arch="$1"
    local triple
    triple="$(get_triple "$arch")"
    local bin_dir="$PROJECT_DIR/target/$triple/release"

    log "Compiling $PKG_NAME for $triple …"

    if $USE_CROSS; then
        require_cmd cross "Install with: cargo install cross"
        cross build --release --target "$triple" --manifest-path "$PROJECT_DIR/Cargo.toml"
    else
        # Ensure the target toolchain is installed
        rustup target add "$triple" 2>/dev/null || true
        cargo build --release --target "$triple" --manifest-path "$PROJECT_DIR/Cargo.toml"
    fi

    echo "$bin_dir/$PKG_NAME"
}

# ---------------------------------------------------------------------------
# DEB packaging (dpkg-deb)
# ---------------------------------------------------------------------------
build_deb() {
    local arch="$1"
    local triple
    triple="$(get_triple "$arch")"
    local bin_path
    bin_path="$(build_binary "$arch")"

    require_cmd dpkg-deb "Install: sudo apt-get install dpkg-dev"

    local deb_arch
    case "$arch" in
        x64)   deb_arch="amd64" ;;
        arm64) deb_arch="arm64" ;;
    esac

    local pkg_root="$RELEASE_DIR/deb-build-${arch}"
    local install_prefix="$pkg_root/usr/bin"
    local service_dir="$pkg_root/lib/systemd/system"
    local config_dir="$pkg_root/etc/$PKG_NAME"
    local debian_dir="$pkg_root/DEBIAN"

    mkdir -p "$install_prefix" "$service_dir" "$config_dir" "$debian_dir"

    # Binary
    cp "$bin_path" "$install_prefix/$PKG_NAME"
    chmod 755 "$install_prefix/$PKG_NAME"

    # Service file
    if [[ -f "$SERVICE_FILE" ]]; then
        cp "$SERVICE_FILE" "$service_dir/${PKG_NAME}.service"
    else
        warn "Service file not found at $SERVICE_FILE – skipping"
    fi

    # Example config
    if [[ -f "$PROJECT_DIR/example.settings.toml" ]]; then
        cp "$PROJECT_DIR/example.settings.toml" "$config_dir/settings.toml.example"
    fi

    # control file
    cat > "$debian_dir/control" <<EOF
Package: $PKG_NAME
Version: $PKG_VERSION
Architecture: $deb_arch
Maintainer: $PKG_MAINTAINER
Description: $PKG_DESCRIPTION
 Bridges a Speeduino ECU (serial or TCP/IP) to an MQTT broker.
 Runs as a systemd service in production or as an interactive TUI.
Section: misc
Priority: optional
Homepage: $PKG_URL
EOF

    # postinst – enable service after install
    cat > "$debian_dir/postinst" <<'EOF'
#!/bin/bash
set -e
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload
    systemctl enable speeduino-to-mqtt.service || true
    echo "Service installed. Edit /etc/speeduino-to-mqtt/settings.toml, then:"
    echo "  sudo systemctl start speeduino-to-mqtt"
fi
exit 0
EOF
    chmod 755 "$debian_dir/postinst"

    # prerm – stop and disable service before removal
    cat > "$debian_dir/prerm" <<'EOF'
#!/bin/bash
set -e
if command -v systemctl >/dev/null 2>&1; then
    systemctl stop  speeduino-to-mqtt.service 2>/dev/null || true
    systemctl disable speeduino-to-mqtt.service 2>/dev/null || true
fi
exit 0
EOF
    chmod 755 "$debian_dir/prerm"

    mkdir -p "$DEB_DIR"
    local deb_file="$DEB_DIR/${PKG_NAME}_${PKG_VERSION}_${deb_arch}.deb"
    dpkg-deb --build "$pkg_root" "$deb_file"
    rm -rf "$pkg_root"

    log "DEB created: $deb_file"
}

# ---------------------------------------------------------------------------
# RPM packaging (rpmbuild)
# ---------------------------------------------------------------------------
build_rpm() {
    local arch="$1"
    local triple
    triple="$(get_triple "$arch")"
    local bin_path
    bin_path="$(build_binary "$arch")"

    require_cmd rpmbuild "Install (Fedora/RHEL): sudo dnf install rpm-build"

    local rpm_arch
    case "$arch" in
        x64)   rpm_arch="x86_64" ;;
        arm64) rpm_arch="aarch64" ;;
    esac

    # Full rpmbuild directory tree – all standard subdirs must exist
    local build_root="$RELEASE_DIR/rpm-build-${arch}"
    local rpm_sources="$build_root/SOURCES"
    local rpm_specs="$build_root/SPECS"
    mkdir -p "$rpm_sources" "$rpm_specs" \
             "$build_root/BUILD" "$build_root/BUILDROOT" \
             "$build_root/RPMS"  "$build_root/SRPMS"

    # Stage files into SOURCES so the spec %install section can find them
    cp "$bin_path" "$rpm_sources/$PKG_NAME"
    [[ -f "$SERVICE_FILE" ]] && cp "$SERVICE_FILE" "$rpm_sources/${PKG_NAME}.service"
    [[ -f "$PROJECT_DIR/example.settings.toml" ]] && \
        cp "$PROJECT_DIR/example.settings.toml" "$rpm_sources/settings.toml.example"

    local spec_file="$rpm_specs/${PKG_NAME}.spec"
    # Variables ($PKG_NAME etc.) are expanded by bash; RPM macros (%{...}) pass through unchanged.
    cat > "$spec_file" <<EOF
Name:           $PKG_NAME
Version:        $PKG_VERSION
Release:        1%{?dist}
Summary:        $PKG_DESCRIPTION
License:        $PKG_LICENSE
URL:            $PKG_URL
BuildArch:      $rpm_arch

# Suppress debuginfo/debugsource packages for pre-built cross-compiled binaries
%global debug_package %{nil}

# Explicit systemd unit path – not always provided as a macro on every distro
%define _unitdir /usr/lib/systemd/system

%description
Bridges a Speeduino ECU (serial or TCP/IP) to an MQTT broker.
Runs as a systemd service in production or as an interactive TUI.

%prep
# Nothing to unpack – binary is shipped pre-built in SOURCES.

%build
# Nothing to compile – using the pre-built binary from SOURCES.

%install
install -Dm755 %{_sourcedir}/$PKG_NAME %{buildroot}%{_bindir}/$PKG_NAME
install -Dm644 %{_sourcedir}/${PKG_NAME}.service %{buildroot}%{_unitdir}/${PKG_NAME}.service
install -Dm644 %{_sourcedir}/settings.toml.example %{buildroot}%{_sysconfdir}/$PKG_NAME/settings.toml.example

%files
%{_bindir}/$PKG_NAME
%{_unitdir}/${PKG_NAME}.service
%dir %{_sysconfdir}/$PKG_NAME
%config(noreplace) %{_sysconfdir}/$PKG_NAME/settings.toml.example

%post
# systemd-rpm-macros provides these helpers on Fedora/RHEL
%systemd_post ${PKG_NAME}.service
echo "Service installed. Copy and edit the config, then start:"
echo "  sudo cp /etc/$PKG_NAME/settings.toml.example /etc/$PKG_NAME/settings.toml"
echo "  sudo systemctl start $PKG_NAME"

%preun
%systemd_preun ${PKG_NAME}.service

%postun
%systemd_postun_with_restart ${PKG_NAME}.service
EOF

    mkdir -p "$RPM_DIR"
    # --target sets the RPM architecture header for cross-compiled packages.
    # rpmbuild writes output to RPMS/<arch>/ inside _topdir; copy to RPM_DIR afterwards.
    rpmbuild \
        --define "_topdir $build_root" \
        --target "$rpm_arch" \
        -bb "$spec_file"

    find "$build_root/RPMS" -name '*.rpm' -exec cp {} "$RPM_DIR/" \;
    rm -rf "$build_root"
    log "RPM created in: $RPM_DIR"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
mkdir -p "$DEB_DIR" "$RPM_DIR"

for arch in "${TARGETS[@]}"; do
    log "=== Architecture: $arch ==="

    case "$PKG_TYPE" in
        deb) build_deb "$arch" ;;
        rpm) build_rpm "$arch" ;;
        all)
            build_deb "$arch" || warn "DEB build failed for $arch"
            build_rpm "$arch" || warn "RPM build failed for $arch"
            ;;
        *) die "Unknown package type '$PKG_TYPE'. Use deb, rpm, or all." ;;
    esac
done

log "=== Build complete. Packages in $RELEASE_DIR ==="
find "$RELEASE_DIR" -name "*.deb" -o -name "*.rpm" 2>/dev/null | sort | while read -r f; do
    echo "  $f"
done
