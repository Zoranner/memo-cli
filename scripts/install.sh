#!/usr/bin/env bash
set -euo pipefail

REPO="Zoranner/memo-cli"
INSTALL_DIR="${MEMO_INSTALL_DIR:-$HOME/.memo/bin}"

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) echo "x86_64-unknown-linux-musl" ;;
        *)
          echo "Unsupported Linux architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64) echo "x86_64-apple-darwin" ;;
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        *)
          echo "Unsupported macOS architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "Unsupported operating system: $os" >&2
      exit 1
      ;;
  esac
}

download_url() {
  local asset="$1"
  echo "https://github.com/$REPO/releases/latest/download/$asset"
}

ensure_path_hint() {
  local shell_rc
  if [[ ":$PATH:" == *":$INSTALL_DIR:"* ]]; then
    return
  fi

  for shell_rc in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile"; do
    if [[ -f "$shell_rc" || "$shell_rc" == "$HOME/.profile" ]]; then
      if ! grep -Fq "$INSTALL_DIR" "$shell_rc" 2>/dev/null; then
        {
          echo ""
          echo "# Added by memo installer"
          echo "export PATH=\"$INSTALL_DIR:\$PATH\""
        } >> "$shell_rc"
        echo "Added $INSTALL_DIR to PATH in $shell_rc"
      fi
      return
    fi
  done
}

main() {
  local target asset url temp_dir archive_path extracted_binary final_binary
  target="$(detect_target)"
  asset="memo-${target}.tar.gz"
  url="$(download_url "$asset")"

  temp_dir="$(mktemp -d)"
  trap 'rm -rf "$temp_dir"' EXIT
  archive_path="$temp_dir/$asset"

  echo "Downloading $asset from $url"
  curl -fsSL "$url" -o "$archive_path"

  mkdir -p "$INSTALL_DIR"
  tar -xzf "$archive_path" -C "$temp_dir"
  extracted_binary="$temp_dir/memo"
  final_binary="$INSTALL_DIR/memo"
  install -m 755 "$extracted_binary" "$final_binary"

  ensure_path_hint

  echo "Installed memo to $final_binary"
  if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "Restart your shell or run: export PATH=\"$INSTALL_DIR:\$PATH\""
  fi
  echo "Then run: memo awaken"
}

main "$@"
