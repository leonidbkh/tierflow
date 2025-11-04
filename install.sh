#!/bin/sh
# Tierflow installer script
# Inspired by rustup, deno, and starship installers

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
        error "Tierflow is designed for standard Linux servers."
        exit 1
    fi

    PLATFORM="x86_64-unknown-linux-gnu"
    info "Platform: Linux x86_64"
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

    if ! command -v rsync >/dev/null 2>&1; then
        warn "rsync is not installed - required for tierflow to move files"
        warn "Install with: apt install rsync (Ubuntu/Debian) or brew install rsync (macOS)"
    fi
}

# Get latest version from GitHub
get_latest_version() {
    info "Fetching latest version..."

    VERSION=$(curl -sSf https://api.github.com/repos/leonidbkh/tierflow/releases/latest \
        | grep '"tag_name":' \
        | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        error "Failed to fetch latest version"
        exit 1
    fi

    info "Latest version: $VERSION"
}

# Download and extract binary
download_binary() {
    local download_url tmp_dir

    BINARY_NAME="tierflow-${PLATFORM}"
    download_url="https://github.com/leonidbkh/tierflow/releases/download/${VERSION}/${BINARY_NAME}.tar.gz"

    info "Downloading from: $download_url"

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

# Install binary
install_binary() {
    local install_dir

    # Try to install to /usr/local/bin first (system-wide)
    if [ -w "/usr/local/bin" ]; then
        install_dir="/usr/local/bin"
    # If not writable, try ~/.local/bin (user-local)
    elif mkdir -p "$HOME/.local/bin" 2>/dev/null; then
        install_dir="$HOME/.local/bin"

        # Add to PATH if not already there
        case ":$PATH:" in
            *":$install_dir:"*) ;;
            *)
                warn "$install_dir is not in your PATH"
                info "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
                printf '  %s\n' "export PATH=\"\$HOME/.local/bin:\$PATH\""
                ;;
        esac
    else
        error "Cannot find suitable installation directory"
        error "Please install manually or run with sudo"
        exit 1
    fi

    info "Installing to $install_dir..."

    if ! mv "$BINARY_PATH" "$install_dir/tierflow"; then
        error "Failed to install binary"
        error "You may need to run this script with sudo"
        exit 1
    fi

    success "Tierflow installed to $install_dir/tierflow"
}

# Verify installation
verify_installation() {
    if ! command -v tierflow >/dev/null 2>&1; then
        warn "tierflow is not in PATH, but was installed successfully"
        warn "You may need to restart your shell or add the install directory to PATH"
        return
    fi

    local installed_version
    installed_version=$(tierflow --version 2>/dev/null | awk '{print $2}' || echo "unknown")

    success "Installation verified!"
    info "Installed version: $installed_version"
}

# Install default config
install_config() {
    local config_dir="/etc/tierflow"
    local config_file="$config_dir/config.yaml"

    info "Installing default configuration..."

    # Try to create config directory
    if [ -w "/etc" ] || sudo mkdir -p "$config_dir" 2>/dev/null; then
        # Download safe default config
        if ! sudo curl -sSfL "https://raw.githubusercontent.com/leonidbkh/tierflow/main/config.yaml" -o "$config_file" 2>/dev/null; then
            warn "Failed to download config, creating minimal one"
            sudo tee "$config_file" >/dev/null <<'EOF'
# Tierflow Configuration
# IMPORTANT: This config does nothing by default - edit it first!
# See examples: https://github.com/leonidbkh/tierflow/blob/main/config.example.yaml

tiers: []
strategies: []
EOF
        fi
        success "Config installed to $config_file"
        info "IMPORTANT: Edit this file to define your tiers and strategies"
    else
        warn "Cannot create $config_dir (no permissions)"
        info "Create it manually: sudo mkdir -p $config_dir"
    fi
}

# Ask about systemd installation
ask_install_systemd() {
    printf '\n%s' "${BOLD}Install systemd service?${RESET} (requires sudo) [y/N]: "
    read -r answer

    case "$answer" in
        [yY]|[yY][eE][sS])
            install_systemd
            ;;
        *)
            info "Skipping systemd installation"
            info "You can install it later: sudo cp tierflow.service /etc/systemd/system/"
            ;;
    esac
}

# Install systemd service
install_systemd() {
    local service_file="/etc/systemd/system/tierflow.service"

    info "Installing systemd service..."

    # Download service file
    if ! sudo curl -sSfL "https://raw.githubusercontent.com/leonidbkh/tierflow/main/tierflow.service" -o "$service_file" 2>/dev/null; then
        error "Failed to download service file"
        return 1
    fi

    # Reload systemd
    if command -v systemctl >/dev/null 2>&1; then
        sudo systemctl daemon-reload
        success "Systemd service installed to $service_file"
        info "Enable and start: sudo systemctl enable --now tierflow"
        info "IMPORTANT: Edit /etc/tierflow/config.yaml first!"
    else
        warn "systemctl not found"
    fi
}

# Print next steps
print_next_steps() {
    printf '\n%s\n' "${BOLD}Next steps:${RESET}"
    printf '  1. Edit config: %s\n' "${BLUE}sudo nano /etc/tierflow/config.yaml${RESET}"
    printf '  2. Check examples: %s\n' "${BLUE}https://github.com/leonidbkh/tierflow/blob/main/config.example.yaml${RESET}"
    printf '  3. Test with dry-run: %s\n' "${BLUE}tierflow rebalance --config /etc/tierflow/config.yaml --dry-run${RESET}"
    printf '  4. Start daemon: %s\n' "${BLUE}sudo systemctl enable --now tierflow${RESET}"
    printf '\n'
}

# Main installation flow
main() {
    info "Installing Tierflow..."
    printf '\n'

    check_requirements
    detect_platform
    get_latest_version
    download_binary
    install_binary
    verify_installation

    printf '\n'
    install_config
    ask_install_systemd

    printf '\n'
    success "Tierflow has been installed successfully!"
    print_next_steps
}

# Run main function
main "$@"
