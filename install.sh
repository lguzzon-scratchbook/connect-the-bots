#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
BIN_NAME="pas"
PROFILE="${PAS_PROFILE:-dist}"

# Check for fast build tools and suggest setup
if [ "${1:-}" = "--fast" ] || [ "${1:-}" = "-f" ]; then
    PROFILE="quick-release"
    echo "Using quick-release profile for faster build..."
else
    echo "Building Pascal's Discrete Attractor (profile: ${PROFILE})..."
    echo "For faster builds (90% optimized), use: ./install.sh --fast"
fi

# Run fast-build setup if sccache not available
if [ -z "${RUSTC_WRAPPER:-}" ] && ! command -v sccache &> /dev/null; then
    echo "Tip: Run ./scripts/setup-fast-builds.sh once for 2-3x faster incremental builds"
fi

cargo build -p attractor-cli --profile "${PROFILE}"

# Create install directory if needed
mkdir -p "${INSTALL_DIR}"

# Copy binary from correct profile directory
cp "target/${PROFILE}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"

# Sign with Apple Developer identity (prevents macOS SIGKILL on unsigned binaries)
if [[ -z "${CODESIGN_IDENTITY:-}" ]]; then
	echo "WARNING: CODESIGN_IDENTITY not set, skipping codesign"
elif [[ "$OSTYPE" == darwin* ]]; then
	if security find-identity -v -p codesigning 2>/dev/null | grep -q "${CODESIGN_IDENTITY}"; then
		codesign --force --sign "${CODESIGN_IDENTITY}" "${INSTALL_DIR}/${BIN_NAME}"
		echo "Signed ${BIN_NAME} with ${CODESIGN_IDENTITY}"
	else
		echo "WARNING: Signing identity not found, skipping codesign"
	fi
fi

echo "Installed ${BIN_NAME} to ${INSTALL_DIR}/${BIN_NAME}"

# Check if install dir is in PATH
if ! echo "${PATH}" | tr ':' '\n' | grep -qx "${INSTALL_DIR}"; then
	echo ""
	echo "WARNING: ${INSTALL_DIR} is not in your PATH."
	echo "Add it to your shell config:"
	echo ""
	echo "  # bash (~/.bashrc)"
	echo "  export PATH=\"${INSTALL_DIR}:\${PATH}\""
	echo ""
	echo "  # zsh (~/.zshrc)"
	echo "  export PATH=\"${INSTALL_DIR}:\${PATH}\""
	echo ""
	echo "  # fish (~/.config/fish/config.fish)"
	echo "  fish_add_path ${INSTALL_DIR}"
fi

echo ""
"${INSTALL_DIR}/${BIN_NAME}" --version
