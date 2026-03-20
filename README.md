# ZunduxTTS

VOICEVOX の音声合成を VRChat の仮想マイクとして使える Linux デスクトップアプリです。テキストを入力すると VOICEVOX で音声合成し、PulseAudio の仮想シンクを経由して VRChat にマイク入力として送信します。

## 機能

- **テキスト音声合成** — テキスト入力 → VOICEVOX で合成 → 仮想マイクに出力
- **テンプレート** — よく使うフレーズをワンクリックで送信
- **マイク切替** — ずんだもんの声と自分のマイクをワンクリックで切り替え
- **停止ボタン** — 合成中・再生中の音声を即座に停止
- **サウンドボード** — フォルダ内の音声ファイルをワンクリック再生
- **読み上げ辞書** — VOICEVOX ユーザー辞書 + サイレント辞書（特定の単語を読み上げない）
- **OSC チャットボックス** — VRChat の OSC 経由でチャットボックスにテキストを表示
- **音声エフェクト** — エコー（ディレイ・減衰の調整）
- **テーマカスタマイズ** — 各UI要素の色、ウィンドウ・タイトルバーの透明度を自由に変更
- **VOICEVOX 自動起動** — Docker コンテナまたはローカルバイナリを自動で起動・管理

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

> **注意**: `--rm` は付けないでください。付けるとコンテナ停止時に削除され、毎回新規作成になります。コンテナは `docker stop` / `docker start` で再利用されるため、`docker run` は初回のみで済みます。

起動後、`http://127.0.0.1:50021` で API が利用可能になります。

### 3. ビルド

```bash
git clone <リポジトリURL>
cd zundamon_vrc
cargo build --release
```

ビルド成果物は `target/release/zundux_tts` に生成されます。

### 4. デスクトップエントリの登録 (任意)

アプリケーションランチャーから起動したい場合は、`.desktop` ファイルをコピーします。

```bash
# Exec= のパスを自分の環境に合わせて編集してください
cp zundux_tts.desktop ~/.local/share/applications/
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
./target/release/zundux_tts
# または
cargo run --release
```

### 自動起動の設定

アプリと VOICEVOX の両方を PC 起動時に自動的に起動できます。

**1. VOICEVOX 自動起動の設定**

アプリの「設定」タブ → 「起動設定」セクションで **「アプリ起動時にVOICEVOXを自動起動」** にチェックを入れると、アプリ起動時に停止中の Docker コンテナを自動で `docker start` します。

バイナリ版を使う場合は **VOICEVOX実行パス** にパスを入力してください（例: `/path/to/voicevox_engine/run`）。

**2. アプリ自動起動の設定 (XDG autostart)**

「設定」タブ → 「起動設定」で **「PC起動時にアプリを自動起動」** にチェックを入れると、`~/.config/autostart/zundux_tts.desktop` が自動で作成され、ログイン時にアプリが起動します。

> 自動起動の `.desktop` ファイルは現在の実行ファイルのパスを参照します。`cargo build --release` でビルドした場合は `target/release/zundux_tts` が登録されます。

### 初回セットアップ

1. アプリが起動したら、ステータスバーで VOICEVOX の接続状態を確認します。「未接続」と表示されている場合は設定タブから接続先 URL を確認してください。
2. PulseAudio 仮想シンクはアプリ起動時に自動作成されます。`ZundamonVRC.monitor` というマイクソースが利用可能になります。

### VRChat 側の設定

VRChat のマイク設定で `ZundamonVRC.monitor`（または PulseAudio の設定で「ZundamonVRC_mic」として表示されるソース）を入力デバイスとして選択してください。

## 使い方

### テキスト入力

- テキストボックスにテキストを入力して **Enter** で送信（音声合成 → 再生）
- **Shift+Enter** で改行
- 合成中は **STOP** ボタンで停止可能

### マイク切替

入力画面の **MIC** ボタンで、ずんだもんの声と自分のマイクを切り替えられます。

- **MIC: OFF** (デフォルト) — テキスト入力した内容をずんだもんの声で出力
- **MIC: ON** — 自分のマイクの音声をそのまま仮想デバイスに流す

設定タブの「オーディオ」からマイクソースを選択できます。

### テンプレート

よく使うフレーズをテンプレートとして登録できます。ボタンをクリックするだけで即座に音声を合成・送信します。デフォルトでは以下が登録されています:

- こんにちは！
- ありがとう！
- おつかれさまなのだ！
- 了解なのだ！

### サウンドボード

設定タブでフォルダを指定すると、フォルダ内の音声ファイルをワンクリックで再生できます。効果音やBGMの再生に便利です。

### 読み上げ辞書

- **VOICEVOX ユーザー辞書** — VOICEVOX 側に登録された辞書エントリを表示・管理
- **サイレント辞書** — 特定の単語を読み上げ対象から除外（テキストは送信するが音声合成しない）

### OSC チャットボックス

VRChat の OSC 機能を使って、送信テキストをチャットボックスにも表示できます。設定タブの「OSC設定」で有効化し、アドレスとポートを設定してください。

### 音声エフェクト

設定タブの「音声エフェクト」からエコーを有効化できます。ディレイ（ms）と減衰を調整できます。

### 設定項目

| 項目 | 説明 |
|---|---|
| VOICEVOX URL | VOICEVOX Engine の接続先 (デフォルト: `http://127.0.0.1:50021`) |
| VOICEVOX実行パス | ローカルバイナリのパス（Docker の場合は空でOK） |
| VOICEVOX自動起動 | アプリ起動時に VOICEVOX を自動的に起動する |
| アプリ自動起動 | PC ログイン時にアプリを自動起動 (XDG autostart) |
| スピーカー | 使用する音声キャラクター・スタイル (デフォルト: ずんだもん ノーマル) |
| 速度 / ピッチ / 抑揚 / 音量 | 音声合成パラメータの調整 |
| オーディオモニタリング | 合成音声を自分でも聴く |
| マイクソース | マイク切替時に使用するマイクデバイス |
| OSC チャットボックス | VRChat OSC 経由でテキストをチャットボックスに表示 |
| エコー | ディレイとアテネの調整 |
| サウンドボード | 音声ファイルフォルダの指定 |
| 仮想デバイス名 | PulseAudio に作成するシンクの名前 (デフォルト: `ZundamonVRC`) |
| テーマ | 各UI要素の色（#RRGGBB / #RRGGBBAA）、ウィンドウ・タイトルバーの透明度 |

設定は `~/.config/zundamon_vrc/config.toml` に自動保存されます。

## トラブルシューティング

### VOICEVOX に接続できない

- VOICEVOX Engine が起動しているか確認してください: `curl http://127.0.0.1:50021/version`
- Docker の場合はコンテナが動いているか確認: `docker ps`
- 停止中のコンテナがある場合は再起動: `docker start zundux-voicevox`
- 設定タブで URL が正しいか確認してください

### Docker で GPU エラーが出る

`could not select device driver "" with capabilities: [[gpu]]` というエラーの場合:

1. NVIDIA ドライバーが入っているか確認: `nvidia-smi`
2. nvidia-container-toolkit をインストール:
   - Arch / Manjaro: `yay -S nvidia-container-toolkit`
   - Ubuntu / Debian: [NVIDIA 公式ガイド](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html)
3. Docker を再起動: `sudo systemctl restart docker`
4. GPU なしで使う場合は Docker コマンドから `--gpus all` を外し、イメージを `voicevox/voicevox_engine:latest` に変更

### Docker コンテナの再作成

設定を変更したい場合（ポート番号やGPU設定など）は、一度コンテナを削除して再作成してください:

```bash
docker stop zundux-voicevox
docker rm zundux-voicevox
docker run --gpus all -p 50021:50021 --name zundux-voicevox voicevox/voicevox_engine:nvidia-latest
```

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
