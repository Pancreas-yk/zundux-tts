# ずんだもん VRC

VOICEVOX の音声合成を VRChat の仮想マイクとして使える Linux デスクトップアプリです。テキストを入力すると VOICEVOX で音声合成し、PulseAudio の仮想シンクを経由して VRChat にマイク入力として送信します。

## 必要な環境

- **Linux** (PulseAudio または PipeWire+pipewire-pulse が動作する環境)
- **Rust** (ビルドに必要 / 1.70 以上推奨)
- **PulseAudio** (`pactl`, `paplay` コマンドが使えること)
- **VOICEVOX Engine** (音声合成サーバー / ローカルバイナリまたは Docker)
- **Docker** (VOICEVOX を Docker で動かす場合)
- **NVIDIA Container Toolkit** (Docker + GPU を使う場合)
- **Noto Sans CJK フォント** (日本語表示用 / 任意だが強く推奨)

## インストール

### 1. 依存パッケージの導入

#### Arch Linux / Manjaro

```bash
sudo pacman -S pulseaudio rust noto-fonts-cjk
```

#### Ubuntu / Debian

```bash
sudo apt install pulseaudio libpulse0 cargo fonts-noto-cjk
```

#### Fedora

```bash
sudo dnf install pulseaudio rust cargo google-noto-sans-cjk-fonts
```

### 2. VOICEVOX Engine の準備

VOICEVOX Engine をローカルで動作させる必要があります。以下のいずれかの方法で用意してください。

**方法A: 公式バイナリ**

[VOICEVOX 公式サイト](https://voicevox.hiroshiba.jp/) からダウンロードし、任意の場所に展開します。

**方法B: Docker (CPU)**

```bash
docker pull voicevox/voicevox_engine:latest
docker run -p 50021:50021 --name zundux-voicevox voicevox/voicevox_engine:latest
```

**方法C: Docker (NVIDIA GPU — 推奨)**

GPU を使うと音声合成が大幅に高速化されます。事前に NVIDIA Container Toolkit のセットアップが必要です。

```bash
# 1. NVIDIA Container Toolkit をインストール
#    Arch Linux / Manjaro (AUR):
yay -S nvidia-container-toolkit
#    Ubuntu / Debian:
#    https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html の手順に従う

# 2. Docker デーモンを再起動
sudo systemctl restart docker

# 3. GPU が認識されるか確認
docker run --rm --gpus all nvidia/cuda:12.0.0-base-ubuntu22.04 nvidia-smi

# 4. VOICEVOX Engine (GPU版) を取得・起動
docker pull voicevox/voicevox_engine:nvidia-latest
docker run --gpus all -p 50021:50021 --name zundux-voicevox voicevox/voicevox_engine:nvidia-latest
```

> **注意**: `docker: Error response from daemon: could not select device driver "" with capabilities: [[gpu]]` というエラーが出る場合は、nvidia-container-toolkit がインストールされていないか、Docker の再起動が必要です。

起動後、`http://127.0.0.1:50021` で API が利用可能になります。

### 3. ビルド

```bash
git clone <リポジトリURL>
cd zundamon_vrc
cargo build --release
```

ビルド成果物は `target/release/zundamon_vrc` に生成されます。

### 4. デスクトップエントリの登録 (任意)

アプリケーションランチャーから起動したい場合は、`.desktop` ファイルをコピーします。

```bash
# Exec= のパスを自分の環境に合わせて編集してください
cp zundamon_vrc.desktop ~/.local/share/applications/
```

## 起動方法

### 手動起動

```bash
# 1. VOICEVOX Engine を起動 (別ターミナルで)
#    Docker (初回作成済みのコンテナを起動):
docker start zundux-voicevox
#    バイナリ:
/path/to/voicevox_engine/run

# 2. アプリを起動
./target/release/zundamon_vrc
# または
cargo run --release
```

### 自動起動の設定

アプリと VOICEVOX の両方を PC 起動時に自動的に起動できます。

**1. VOICEVOX Docker コンテナの初回作成**

初回のみ以下のコマンドでコンテナを作成してください。コンテナは `docker stop` / `docker start` で再利用されるため、`docker run` は一度だけで済みます。

```bash
# GPU版 (推奨)
docker run --gpus all -p 50021:50021 --name zundux-voicevox voicevox/voicevox_engine:nvidia-ubuntu20.04-latest

# CPU版
docker run -p 50021:50021 --name zundux-voicevox voicevox/voicevox_engine:latest
```

> **注意**: `--rm` は付けないでください。付けるとコンテナ停止時に削除され、毎回新規作成になります。

**2. VOICEVOX 自動起動の設定**

アプリの「設定」タブ → 「起動設定」セクションで **「アプリ起動時にVOICEVOXを自動起動」** にチェックを入れると、アプリ起動時に停止中のコンテナを自動で `docker start` します。

バイナリ版を使う場合は **VOICEVOX実行パス** にパスを入力してください（例: `/path/to/voicevox_engine/run`）。

**2. アプリ自動起動の設定 (XDG autostart)**

「設定」タブ → 「起動設定」で **「PC起動時にアプリを自動起動」** にチェックを入れると、`~/.config/autostart/zundamon_vrc.desktop` が自動で作成され、ログイン時にアプリが起動します。

> 自動起動の `.desktop` ファイルは現在の実行ファイルのパスを参照します。`cargo build --release` でビルドした場合は `target/release/zundamon_vrc` が登録されます。

### 初回セットアップ

1. アプリが起動したら、ステータスバーで VOICEVOX の接続状態を確認します。「未接続」と表示されている場合は設定タブから接続先 URL を確認してください。
2. PulseAudio 仮想シンクはアプリ起動時に自動作成されます。`ZundamonVRC.monitor` というマイクソースが利用可能になります。

### VRChat 側の設定

VRChat のマイク設定で `ZundamonVRC.monitor`（または PulseAudio の設定で「ZundamonVRC_mic」として表示されるソース）を入力デバイスとして選択してください。

## 使い方

### テキスト入力

- テキストボックスにテキストを入力して **Enter** で送信（音声合成 → 再生）
- **Shift+Enter** で改行

### テンプレート

よく使うフレーズをテンプレートとして登録できます。ボタンをクリックするだけで即座に音声を合成・送信します。デフォルトでは以下が登録されています:

- こんにちは！
- ありがとう！
- おつかれさまなのだ！
- 了解なのだ！

### 設定項目

| 項目 | 説明 |
|---|---|
| VOICEVOX URL | VOICEVOX Engine の接続先 (デフォルト: `http://127.0.0.1:50021`) |
| VOICEVOX実行パス | バイナリパスまたは Docker コマンド |
| VOICEVOX自動起動 | アプリ起動時に VOICEVOX を自動的に起動する |
| アプリ自動起動 | PC ログイン時にアプリを自動起動 (XDG autostart) |
| スピーカー | 使用する音声キャラクター・スタイル (デフォルト: ずんだもん ノーマル) |
| 速度 / ピッチ / 抑揚 / 音量 | 音声合成パラメータの調整 |
| ウィンドウ透明度 | ウィンドウ背景の透明度 (10%〜100%) |
| タイトルバー透明度 | タイトルバーの透明度 (10%〜100%) |
| 仮想デバイス名 | PulseAudio に作成するシンクの名前 (デフォルト: `ZundamonVRC`) |

設定は `~/.config/zundamon_vrc/config.toml` に自動保存されます。

## トラブルシューティング

### VOICEVOX に接続できない

- VOICEVOX Engine が起動しているか確認してください: `curl http://127.0.0.1:50021/version`
- Docker の場合はコンテナが動いているか確認: `docker ps`
- 設定タブで URL が正しいか確認してください

### Docker で GPU エラーが出る

`could not select device driver "" with capabilities: [[gpu]]` というエラーの場合:

1. NVIDIA ドライバーが入っているか確認: `nvidia-smi`
2. nvidia-container-toolkit をインストール:
   - Arch / Manjaro: `yay -S nvidia-container-toolkit`
   - Ubuntu / Debian: [NVIDIA 公式ガイド](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html)
3. Docker を再起動: `sudo systemctl restart docker`
4. GPU なしで使う場合は Docker コマンドから `--gpus all` を外し、イメージを `voicevox/voicevox_engine:latest` に変更

### 仮想デバイスが作成できない

- PulseAudio が動作しているか確認してください: `pactl info`
- PipeWire 環境の場合は `pipewire-pulse` がインストールされているか確認してください

### 日本語が表示されない

- Noto Sans CJK フォントがインストールされているか確認してください
- フォントは以下のパスのいずれかに配置される必要があります:
  - `/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc`
  - `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`
  - `/usr/share/fonts/noto-cjk-fonts/NotoSansCJK-Regular.ttc`
  - `/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc`

### VRChat でマイクとして認識されない

- アプリの設定タブで仮想デバイスを「作成」済みか確認してください
- VRChat のマイク設定で `ZundamonVRC.monitor` を選択してください
- PulseAudio のボリューム設定 (`pavucontrol`) で仮想デバイスがミュートされていないか確認してください

## ライセンス

VOICEVOX の利用規約に従ってください。各音声キャラクターにはそれぞれ利用規約があります。
