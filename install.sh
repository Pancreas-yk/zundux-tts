#!/usr/bin/env bash
set -euo pipefail

# ===== ZunduxTTS インストーラー (Arch/Manjaro) =====

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_BIN="$HOME/.local/bin"
INSTALL_APPS="$HOME/.local/share/applications"
INSTALL_ICONS="$HOME/.local/share/icons/hicolor/256x256/apps"
CONFIG_DIR="$HOME/.config/zundux_tts"
GITHUB_REPO="ediblepancreas/zundux_tts"

download_binary() {
    info "最新リリースをダウンロード中..."

    local api_url="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
    local release_json
    release_json=$(curl -fsSL "$api_url") || {
        error "リリース情報の取得に失敗しました"
        return 1
    }

    local binary_url
    binary_url=$(echo "$release_json" | grep -o '"browser_download_url":[[:space:]]*"[^"]*zundux_tts-linux-x86_64"' | grep -o 'https://[^"]*')

    # Validate URL pattern
    if ! echo "$binary_url" | grep -qE "^https://github\.com/${GITHUB_REPO}/releases/download/v[0-9]+\.[0-9]+\.[0-9]+/zundux_tts-linux-x86_64$"; then
        error "ダウンロードURLが不正です: $binary_url"
        return 1
    fi

    local checksum_url
    checksum_url=$(echo "$release_json" | grep -o '"browser_download_url":[[:space:]]*"[^"]*SHA256SUMS"' | grep -o 'https://[^"]*')

    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf '$tmpdir'" EXIT

    curl -fsSL "$binary_url" -o "$tmpdir/zundux_tts-linux-x86_64" || {
        error "バイナリのダウンロードに失敗しました"
        return 1
    }

    curl -fsSL "$checksum_url" -o "$tmpdir/SHA256SUMS" || {
        error "チェックサムのダウンロードに失敗しました"
        return 1
    }

    cd "$tmpdir"
    if ! sha256sum -c SHA256SUMS; then
        error "チェックサム検証に失敗しました。ダウンロードが破損している可能性があります"
        return 1
    fi
    cd "$SCRIPT_DIR"

    cp "$tmpdir/zundux_tts-linux-x86_64" "$INSTALL_BIN/zundux_tts"
    chmod +x "$INSTALL_BIN/zundux_tts"
    info "バイナリをインストール: $INSTALL_BIN/zundux_tts"
}

# ---------- Step 1: 依存パッケージのインストール ----------
info "依存パッケージを確認中..."

if [ "${1:-}" = "--from-source" ]; then
    PACKAGES=(base-devel rust docker pulseaudio noto-fonts-cjk ffmpeg)
else
    PACKAGES=(docker pulseaudio noto-fonts-cjk ffmpeg)
fi
MISSING=()

for pkg in "${PACKAGES[@]}"; do
    if ! pacman -Qi "$pkg" &>/dev/null; then
        MISSING+=("$pkg")
    fi
done

if [ ${#MISSING[@]} -gt 0 ]; then
    info "以下のパッケージをインストールします: ${MISSING[*]}"
    sudo pacman -S --needed --noconfirm "${MISSING[@]}"
else
    info "すべての依存パッケージがインストール済みです"
fi

# ---------- Step 2: GPU検出 & VOICEVOXイメージ選択 ----------
VOICEVOX_IMAGE="voicevox/voicevox_engine:latest"
GPU_FLAGS=""

if lspci | grep -qi nvidia; then
    echo ""
    echo -e "${BOLD}NVIDIA GPUが検出されました。GPU版VOICEVOXを使いますか？${NC}"
    echo "GPU版は合成速度が大幅に速くなります。"
    echo "  1) GPU版 (nvidia-container-toolkit が必要です)"
    echo "  2) CPU版"
    echo ""
    read -rp "選択 [1/2] (デフォルト: 1): " gpu_choice
    gpu_choice="${gpu_choice:-1}"

    if [ "$gpu_choice" = "1" ]; then
        VOICEVOX_IMAGE="voicevox/voicevox_engine:nvidia-latest"
        GPU_FLAGS="--gpus all"

        # nvidia-container-toolkit のインストール
        if ! pacman -Qi nvidia-container-toolkit &>/dev/null; then
            info "nvidia-container-toolkit をインストール中..."
            # Try pacman first, fall back to AUR
            if pacman -Si nvidia-container-toolkit &>/dev/null; then
                sudo pacman -S --needed --noconfirm nvidia-container-toolkit
            else
                warn "nvidia-container-toolkit が公式リポジトリにありません"
                warn "AUR からインストールしてください: yay -S nvidia-container-toolkit"
                echo ""
                read -rp "続行しますか？ (y/N): " cont
                if [ "$cont" != "y" ] && [ "$cont" != "Y" ]; then
                    error "インストールを中断しました"
                    exit 1
                fi
            fi
        fi
        info "GPU版VOICEVOXを使用します"
    else
        info "CPU版VOICEVOXを使用します"
    fi
else
    info "NVIDIA GPUが検出されませんでした。CPU版VOICEVOXを使用します"
fi

# ---------- Step 3: Docker設定 ----------
info "Dockerを設定中..."

# Enable and start docker service
if ! systemctl is-active --quiet docker; then
    sudo systemctl enable --now docker
    info "Dockerサービスを起動しました"
fi

# Add user to docker group
if ! groups "$USER" | grep -q '\bdocker\b'; then
    sudo usermod -aG docker "$USER"
    NEED_RELOGIN=true
    warn "ユーザーをdockerグループに追加しました（再ログインが必要です）"
else
    NEED_RELOGIN=false
fi

# Pull VOICEVOX image
info "VOICEVOXイメージをダウンロード中... (時間がかかる場合があります)"
if [ "$NEED_RELOGIN" = true ]; then
    sudo docker pull "$VOICEVOX_IMAGE"
else
    docker pull "$VOICEVOX_IMAGE"
fi

# ---------- Step 4: ビルド or ダウンロード ----------
mkdir -p "$INSTALL_BIN"
mkdir -p "$INSTALL_APPS"
mkdir -p "$CONFIG_DIR"
mkdir -p "$INSTALL_ICONS"

if [ "${1:-}" = "--from-source" ]; then
    info "ソースからビルド中..."
    if ! command -v cargo &>/dev/null; then
        error "Rustツールチェーンが必要です: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    cd "$SCRIPT_DIR"
    cargo build --release
    cp "$SCRIPT_DIR/target/release/zundux_tts" "$INSTALL_BIN/zundux_tts"
    info "バイナリをインストール: $INSTALL_BIN/zundux_tts"
else
    download_binary
fi

# Build docker run command
DOCKER_CMD="docker run --rm ${GPU_FLAGS:+$GPU_FLAGS }-p 50021:50021 $VOICEVOX_IMAGE"

# Create launch script
cat > "$INSTALL_BIN/zundux_tts_launch.sh" << 'LAUNCHER_EOF'
#!/usr/bin/env bash
VOICEVOX_CONTAINER="zundux-voicevox"
LAUNCHER_EOF

# Append image and GPU flags safely
printf 'VOICEVOX_IMAGE=%q\n' "$VOICEVOX_IMAGE" >> "$INSTALL_BIN/zundux_tts_launch.sh"
printf 'GPU_FLAGS=%q\n' "$GPU_FLAGS" >> "$INSTALL_BIN/zundux_tts_launch.sh"

cat >> "$INSTALL_BIN/zundux_tts_launch.sh" << 'LAUNCHER_EOF'

# Start VOICEVOX if not running
if ! docker ps --format '{{.Names}}' | grep -q "^${VOICEVOX_CONTAINER}$"; then
    docker run -d --rm --name "$VOICEVOX_CONTAINER" ${GPU_FLAGS:+$GPU_FLAGS} -p 50021:50021 "$VOICEVOX_IMAGE" >/dev/null 2>&1
fi

# Run the app
"$HOME/.local/bin/zundux_tts"

# Stop VOICEVOX on exit
docker stop "$VOICEVOX_CONTAINER" 2>/dev/null || true
LAUNCHER_EOF

chmod +x "$INSTALL_BIN/zundux_tts_launch.sh"
info "ランチャーをインストール: $INSTALL_BIN/zundux_tts_launch.sh"

# Create .desktop file
cat > "$INSTALL_APPS/zundux_tts.desktop" << DESKTOP_EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=ZunduxTTS
Comment=VOICEVOX TTS virtual microphone
Exec=$INSTALL_BIN/zundux_tts_launch.sh
Icon=zundux_tts
Terminal=false
StartupWMClass=zundux_tts
Categories=AudioVideo;Audio;
DESKTOP_EOF

info "デスクトップエントリをインストール: $INSTALL_APPS/zundux_tts.desktop"

# Install icon
cp "$SCRIPT_DIR/assets/design-1.png" "$INSTALL_ICONS/zundux_tts.png"
info "アイコンをインストール: $INSTALL_ICONS/zundux_tts.png"

# Update icon cache
if command -v gtk-update-icon-cache &>/dev/null; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi

# Update config with VOICEVOX Docker settings
if [ -f "$CONFIG_DIR/config.toml" ]; then
    # Update existing config
    if grep -q 'voicevox_path' "$CONFIG_DIR/config.toml"; then
        sed -i "s|^voicevox_path.*|voicevox_path = \"$DOCKER_CMD\"|" "$CONFIG_DIR/config.toml"
    else
        echo "voicevox_path = \"$DOCKER_CMD\"" >> "$CONFIG_DIR/config.toml"
    fi
    # Set auto_launch_voicevox = true
    if grep -q 'auto_launch_voicevox' "$CONFIG_DIR/config.toml"; then
        sed -i 's|^auto_launch_voicevox.*|auto_launch_voicevox = true|' "$CONFIG_DIR/config.toml"
    else
        echo "auto_launch_voicevox = true" >> "$CONFIG_DIR/config.toml"
    fi
else
    # Create minimal config
    cat > "$CONFIG_DIR/config.toml" << CONFIG_EOF
voicevox_path = "$DOCKER_CMD"
auto_launch_voicevox = true
CONFIG_EOF
fi

info "設定ファイルを更新: $CONFIG_DIR/config.toml"

# ---------- 完了 ----------
echo ""
echo -e "${GREEN}${BOLD}===== インストール完了！ =====${NC}"
echo ""
echo "アプリケーションメニューから「ZunduxTTS」を起動できます。"
echo "またはコマンドラインから: zundux_tts_launch.sh"
echo ""
if [ "$NEED_RELOGIN" = true ]; then
    echo -e "${YELLOW}${BOLD}重要: dockerグループへの追加を反映するため、再ログインしてください。${NC}"
    echo ""
fi
