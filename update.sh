#!/bin/sh
# Tierflow updater script
# Quick update without touching config or systemd

set -e

# Colors for output
if [ -t 1 ]; then
    RED=$(printf '\033[31m')
    GREEN=$(printf '\033[32m')
    YELLOW=$(printf '\033[33m')
    BLUE=$(printf '\033[34m')
    BOLD=$(printf '\033[1m')
    RESET=$(printf '\033[0m')
else
    RED=""
    GREEN=""
    YELLOW=""
    BLUE=""
    BOLD=""
    RESET=""
fi

info() {
    printf '%s\n' "${BLUE}==>${RESET} ${BOLD}$*${RESET}"
}

success() {
    printf '%s\n' "${GREEN}==>${RESET} ${BOLD}$*${RESET}"
}

warn() {
    printf '%s\n' "${YELLOW}Warning:${RESET} $*"
}

error() {
    printf '%s\n' "${RED}Error:${RESET} $*" >&2
}

# Detect platform
detect_platform() {
    local os arch

    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    # Only support Linux x86_64
    if [ "$os" != "linux" ]; then
        error "Only Linux is supported. Detected OS: $os"
        exit 1
    fi

    if [ "$arch" != "x86_64" ] && [ "$arch" != "amd64" ]; then
        error "Only x86_64 architecture is supported. Detected: $arch"
        exit 1
    fi

    PLATFORM="x86_64-unknown-linux-gnu"
}

# Check for required commands
check_requirements() {
    if ! command -v curl >/dev/null 2>&1; then
        error "curl is required but not installed"
        exit 1
    fi

    if ! command -v tar >/dev/null 2>&1; then
        error "tar is required but not installed"
        exit 1
    fi
}

# Find current installation
find_installation() {
    if command -v tierflow >/dev/null 2>&1; then
        INSTALL_PATH=$(command -v tierflow)
        CURRENT_VERSION=$(tierflow --version 2>/dev/null | awk '{print $2}' || echo "unknown")
        info "Found installation: $INSTALL_PATH"
        info "Current version: $CURRENT_VERSION"
    else
        error "tierflow is not installed or not in PATH"
        error "Install first: curl -sSfL https://raw.githubusercontent.com/leonidbkh/tierflow/main/install.sh | sh"
        exit 1
    fi
}

# Get latest version from GitHub
get_latest_version() {
    info "Checking for updates..."

    VERSION=$(curl -sSf https://api.github.com/repos/leonidbkh/tierflow/releases/latest \
        | grep '"tag_name":' \
        | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        error "Failed to fetch latest version"
        exit 1
    fi

    info "Latest version: $VERSION"

    # Check if already up to date
    if [ "$CURRENT_VERSION" = "${VERSION#v}" ]; then
        success "Already up to date!"
        exit 0
    fi
}

# Download and extract binary
download_binary() {
    local download_url tmp_dir

    BINARY_NAME="tierflow-${PLATFORM}"
    download_url="https://github.com/leonidbkh/tierflow/releases/download/${VERSION}/${BINARY_NAME}.tar.gz"

    info "Downloading $VERSION..."

    tmp_dir=$(mktemp -d)
    trap "rm -rf '$tmp_dir'" EXIT INT TERM

    if ! curl -sSfL "$download_url" -o "$tmp_dir/tierflow.tar.gz"; then
        error "Failed to download tierflow"
        error "URL: $download_url"
        exit 1
    fi

    info "Extracting..."
    if ! tar -xzf "$tmp_dir/tierflow.tar.gz" -C "$tmp_dir"; then
        error "Failed to extract archive"
        exit 1
    fi

    BINARY_PATH="$tmp_dir/tierflow"

    if [ ! -f "$BINARY_PATH" ]; then
        error "Binary not found in archive"
        exit 1
    fi

    # Make executable
    chmod +x "$BINARY_PATH"
}

# Update binary
update_binary() {
    info "Updating $INSTALL_PATH..."

    # Try without sudo first
    if mv "$BINARY_PATH" "$INSTALL_PATH" 2>/dev/null; then
        success "Updated to $VERSION"
        return
    fi

    # Try with sudo
    info "Update requires sudo..."
    if ! sudo mv "$BINARY_PATH" "$INSTALL_PATH"; then
        error "Failed to update binary"
        exit 1
    fi

    success "Updated to $VERSION"
}

# Verify installation
verify_installation() {
    local new_version
    new_version=$(tierflow --version 2>/dev/null | awk '{print $2}' || echo "unknown")

    if [ "$new_version" != "${VERSION#v}" ]; then
        warn "Version mismatch: expected ${VERSION#v}, got $new_version"
        warn "You may need to restart your shell"
        return
    fi

    success "Verification passed!"
}

# Restart service if running
restart_service() {
    if command -v systemctl >/dev/null 2>&1; then
        if systemctl is-active --quiet tierflow 2>/dev/null; then
            info "Restarting tierflow service..."
            if sudo systemctl restart tierflow; then
                success "Service restarted"
            else
                warn "Failed to restart service. Restart manually: sudo systemctl restart tierflow"
            fi
        fi
    fi
}

# Main update flow
main() {
    info "Updating Tierflow..."
    printf '\n'

    check_requirements
    detect_platform
    find_installation
    get_latest_version
    download_binary
    update_binary
    verify_installation
    restart_service

    printf '\n'
    success "Tierflow has been updated successfully!"
    info "New version: $VERSION"
}

# Run main function
main "$@"
