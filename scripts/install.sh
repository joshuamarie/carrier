#!/bin/bash
set -e

DEFAULT_VERSION="__VERSION__"

mkdir -p ~/.local/bin && cd ~/.local/bin

os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)

case "$os" in
  darwin) os_pattern="apple-darwin"; ext="tar.gz" ;;
  linux)  os_pattern="unknown-linux-gnu"; ext="tar.gz" ;;
  mingw*|msys*|cygwin*)
    echo "Detected Windows (via Git Bash/MSYS)."
    os_pattern="pc-windows-msvc"
    ext="zip"
    if ! command -v unzip >/dev/null 2>&1; then
      echo "Error: 'unzip' not found in this Git Bash environment." >&2
      exit 1
    fi
    ;;
  *) echo "Unsupported OS: $os" >&2; exit 1 ;;
esac

version="${CARRIER_VERSION:-}"
if [ -z "$version" ] && [ "$DEFAULT_VERSION" != "__VERSION__" ]; then
    version="$DEFAULT_VERSION"
fi

if [ -n "$version" ]; then
    echo "Fetching release $version..."
    api_url="https://api.github.com/repos/joshuamarie/carrier/releases/tags/$version"
else
    echo "Fetching latest release..."
    api_url="https://api.github.com/repos/joshuamarie/carrier/releases/latest"
fi

echo "Fetching download URL for $arch-$os_pattern..."
<<<<<<< HEAD
github_response=$(curl -s "$api_url")
=======
github_response=$(curl -s https://api.github.com/repos/joshuamarie/carrier/releases/latest)
>>>>>>> abb93f70f5f522074ebe094324cc788b98a8099c
asset_url=$(echo "$github_response" | grep -o "https://github.com/joshuamarie/carrier/releases/download/[^\"]*$arch-$os_pattern.$ext\"" | head -n 1)
asset_url="${asset_url%\"}"

if [ -z "$asset_url" ]; then
    echo "Error: no release asset found for $arch-$os_pattern." >&2
    exit 1
fi

echo "Downloading carrier from $asset_url"
if [ "$ext" = "zip" ]; then
    curl -L -o carrier_latest.zip "$asset_url"
    unzip -o carrier_latest.zip -d .
    rm carrier_latest.zip
    mv carrier.exe carrier 2>/dev/null || true
else
    curl -L -o carrier_latest.tar.gz "$asset_url"
    tar -xzf carrier_latest.tar.gz --strip-components=1
    rm carrier_latest.tar.gz
fi
chmod +x carrier* 2>/dev/null || true
echo "carrier installed to ~/.local/bin"

if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    if [[ "$SHELL" == *"bash"* ]]; then
        printf '\n%s\n' 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
    elif [[ "$SHELL" == *"zsh"* ]]; then
        printf '\n%s\n' 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
    elif [[ "$SHELL" == *"fish"* ]]; then
        printf '\n%s\n' 'fish_add_path "$HOME/.local/bin"' >> ~/.config/fish/config.fish
    fi
    echo "Please restart your terminal, or source your shell config."
else
    echo "~/.local/bin is already in your PATH."
fi
