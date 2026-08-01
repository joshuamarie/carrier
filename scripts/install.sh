#!/bin/bash
set -e

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
      echo "Run the Windows installer directly in PowerShell instead:" >&2
      echo "  irm https://raw.githubusercontent.com/joshuamarie/carrier/refs/heads/main/scripts/install.ps1 | iex" >&2
      exit 1
    fi
    ;;
  *) echo "Unsupported OS: $os" >&2; exit 1 ;;
esac

echo "Fetching download URL for $arch-$os_pattern..."
github_response=$(curl -s https://api.github.com/repos/joshuamarie/carrier/releases/latest)
asset_url=$(echo "$github_response" | grep -o "https://github.com/joshuamarie/carrier/releases/download/[^\"]*$arch-$os_pattern.$ext")

if [ -z "$asset_url" ]; then
    echo "Error: no release asset found for $arch-$os_pattern." >&2
    echo "Check https://github.com/joshuamarie/carrier/releases/latest" >&2
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
    echo "Adding ~/.local/bin to your PATH..."
    if [[ "$SHELL" == *"bash"* ]]; then
        printf '\n%s\n' 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
        echo "Please source ~/.bashrc or open a new terminal."
    elif [[ "$SHELL" == *"zsh"* ]]; then
        printf '\n%s\n' 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
        echo "Please source ~/.zshrc or open a new terminal."
    elif [[ "$SHELL" == *"fish"* ]]; then
        printf '\n%s\n' 'fish_add_path "$HOME/.local/bin"' >> ~/.config/fish/config.fish
    else
        echo "Could not detect shell. Add ~/.local/bin to your PATH manually."
    fi
else
    echo "~/.local/bin is already in your PATH."
fi
