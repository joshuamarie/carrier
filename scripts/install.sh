#!/bin/bash
set -e

mkdir -p ~/.local/bin && cd ~/.local/bin

os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)

case "$os" in
  darwin) os_pattern="apple-darwin" ;;
  linux)  os_pattern="unknown-linux-gnu" ;;
  *) echo "Unsupported OS: $os" >&2; exit 1 ;;
esac

echo "Fetching download URL for $arch-$os_pattern..."
github_response=$(curl -s https://api.github.com/repos/joshuamarie/carrier/releases/latest)
asset_url=$(echo "$github_response" | grep -o "https://github.com/joshuamarie/carrier/releases/download/[^\"]*$arch-$os_pattern.tar.gz")

if [ -z "$asset_url" ]; then
    echo "Error: no release asset found for $arch-$os_pattern." >&2
    echo "Check https://github.com/joshuamarie/carrier/releases/latest" >&2
    exit 1
fi

echo "Downloading carrier from $asset_url"
curl -L -o carrier_latest.tar.gz "$asset_url" &&
    tar -xzf carrier_latest.tar.gz --strip-components=1 &&
    rm carrier_latest.tar.gz &&
    chmod +x carrier &&
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
