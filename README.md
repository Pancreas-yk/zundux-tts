# ZunduxTTS

Linux デスクトップで動作する VRChat 向け TTS 仮想マイクアプリです。テキストを入力すると VOICEVOX または Voiceger (GPT-SoVITS) で音声合成し、PulseAudio の仮想シンクを経由して VRChat にマイク入力として送信します。

## 機能

- **2つの TTSエンジン**
  - **VOICEVOX** — 高品質な日本語 TTS。ずんだもんをはじめ多数の音声キャラクター
  - **Voiceger (GPT-SoVITS)** — 日本語・英語・中文・한국어・粤語の5言語対応。GPU 推論でリアルタイムに近い速度
- **スピーカープリセット** — エンジンごとに音声・スタイル・パラメータをプリセット登録。テンプレートとの組み合わせ自由
- **テンプレート** — よく使うフレーズをワンクリックで送信。プリセットを選んで展開も可能
- **マイク切替** — TTS の声と実マイクをワンクリックで切り替え（パススルー）
- **ノイズキャンセル** — RNNoise 対応環境では実マイクのノイズ除去が可能
- **読み上げ辞書**
  - VOICEVOX ユーザー辞書（API 経由）
  - Voiceger クライアント辞書（言語ごとの読み替え）
  - サイレント辞書（両エンジン共通・特定ワードを無音化）
- **OSC チャットボックス** — VRChat の OSC 経由でチャットボックスにテキストを表示
- **サウンドボード** — フォルダ内の音声ファイルをワンクリック再生
- **音声エフェクト** — エコー（ディレイ・減衰の調整）
- **テーマカスタマイズ** — 色・透明度を自由に変更
- **自動起動** — VOICEVOX / Voiceger を アプリ起動時に自動起動・管理
- **言語不一致検出** — Voiceger で選択言語とテキストが一致しない場合に警告

## 必要な環境

### 共通

- **Linux** (PulseAudio または PipeWire + pipewire-pulse が動作する環境)
- **PulseAudio ツール** (`pactl`, `paplay`)
- **ffmpeg** (Voiceger の音声変換・ピッチ/音量調整に使用)
- **Noto Sans CJK フォント** (日本語・CJK 表示用)

### VOICEVOX

- **VOICEVOX Engine** — ローカルバイナリまたは Docker

### Voiceger (任意)

- **Miniconda / Anaconda**
- **NVIDIA GPU** 推奨 (7GB+ VRAM、CUDA 12.x)
- GPT-SoVITS リポジトリ (`~/voiceger_v2` などに配置)

## インストール

### インストールスクリプト (Arch / Manjaro)

依存パッケージ・VOICEVOX Docker・Voiceger のセットアップをまとめて行います。

```bash
git clone https://github.com/Pancreas-yk/zundux-tts
cd zundux-tts

# リリースバイナリをダウンロードして導入
bash install.sh

# ソースからビルドして導入
bash install.sh --from-source
```

スクリプトが行うこと:
- 依存パッケージのインストール (`docker`, `noto-fonts-cjk`, `ffmpeg` など)
- NVIDIA GPU 検出 → GPU 版 / CPU 版 VOICEVOX Docker イメージのダウンロード
- Voiceger のインストール（任意選択）:
  - conda 環境 (`voiceger`) の作成
  - PyTorch (CUDA 対応) のインストール
  - GPT-SoVITS 事前学習モデルのダウンロード
  - ずんだもん Fine-tuned モデルのダウンロード
  - G2PWModel（中国語ピンイン推論）のダウンロード
  - GPU モード (`device: cuda`) の設定
- デスクトップエントリ・アイコンの登録

### アンインストール

```bash
bash install.sh --uninstall
```

### 手動ビルド

```bash
cargo build --release
# バイナリ: target/release/zundux_tts
```

## 初回セットアップ

### VRChat 側の設定

アプリを起動すると `ZunduxMic` という仮想シンクと `ZunduxMic_mic` という仮想ソースが PulseAudio に作成されます。

VRChat のマイク設定で **`ZunduxMic_mic`**（または `ZunduxMic.monitor`）を入力デバイスとして選択してください。

> 仮想デバイスはアプリ終了後も PipeWire セッション中は保持されます。アプリを再起動してもマイクのトグルは不要です。ただしログアウト・再起動後は初回のみ再選択が必要です。

### VOICEVOX の設定

設定 → General → 起動設定:
- **VOICEVOXを自動起動**: ON にすると起動時に Docker コンテナ / ローカルバイナリを自動起動
- **VOICEVOX実行パス**: Docker を使う場合は `docker run --rm -p 50021:50021 voicevox/voicevox_engine:nvidia-latest` など

### Voiceger の設定

設定 → Voiceger → 接続:
- **起動コマンド**: `conda run -n voiceger python /path/to/GPT-SoVITS/api_v2.py` など
- **参照音声**: ずんだもんの参照音声ファイル (`.wav`)
- **参照テキスト** / **参照言語**: 参照音声の書き起こしと言語
- **参照なしモード (ref_free)**: 参照音声を使わず合成（短い英字入力は自動で ref_free）

Voiceger は言語ごとに**プリセット**が必要です（設定 → Voiceger → プリセット）。
Voiceger プリセットでは **参照WAV（任意）** を設定できます（優先順: 参照WAV > 感情 > グローバル参照音声）。

## 使い方

### テキスト入力

- テキストボックスに入力して **Enter** で送信（Shift+Enter で改行）
- **STOP** ボタンで再生を即座に停止
- プリセットチップをクリックして音声・言語を切り替え

### テンプレート

Input タブのテンプレートボタンをクリックで即座に合成・送信。設定 → General → テンプレートで編集・並び替え可能。

### マイク切替

**MIC** ボタンで TTS 出力と実マイクのパススルーを切り替え。設定 → Audio → マイクソースでデバイスを選択。

### サウンドボード

設定 → Audio → サウンドボード でフォルダを指定（参照ボタンあり）。Input タブのサウンドボードパネルからワンクリック再生。

### OSC チャットボックス

設定 → General → OSC設定 で有効化。デフォルトは `127.0.0.1:9000`（VRChat デフォルト）。

### 読み上げ辞書

| 辞書 | 場所 | 内容 |
|---|---|---|
| VOICEVOX辞書 | 設定 → VOICEVOX → 辞書 | VOICEVOX API に登録される読み替えルール |
| Voiceger辞書 | 設定 → Voiceger → 辞書 | 言語ごとのクライアント辞書（合成前に置換） |
| サイレント辞書 | 設定 → Voiceger → 辞書 | 両エンジン共通・特定ワードを無音化 |

## 設定項目

設定は `~/.config/zundux_tts/config.toml` に自動保存されます。

### General

| 項目 | 説明 |
|---|---|
| VOICEVOX自動起動 | アプリ起動時に VOICEVOX を自動起動 |
| Voiceger自動起動 | アプリ起動時に Voiceger を自動起動 |
| アプリ自動起動 | ログイン時にアプリを自動起動 (XDG autostart) |
| TTSエンジン | VOICEVOX / Voiceger の切り替え（再起動ボタンあり）|
| OSC | チャットボックス送信の有効化・アドレス・ポート |
| テンプレート | フレーズの追加・編集・削除 |
| テーマ | 色・透明度のカスタマイズ |

### VOICEVOX

| 項目 | 説明 |
|---|---|
| 接続URL | VOICEVOX Engine のアドレス（デフォルト: `http://127.0.0.1:50021`）|
| 実行パス | バイナリ or Docker コマンド |
| 音声パラメータ | 速度・ピッチ・抑揚・音量 |
| プリセット | スピーカー・スタイル・パラメータのセット |
| 辞書 | ユーザー辞書エントリの管理 |

### Voiceger

| 項目 | 説明 |
|---|---|
| 接続URL | GPT-SoVITS API のアドレス（デフォルト: `http://127.0.0.1:9880`）|
| 起動コマンド | サーバー起動コマンド |
| 参照音声 / テキスト / 言語 | Zero-shot 合成のリファレンス |
| 参照なしモード (ref_free) | 参照音声なしで合成（短い英字入力は自動適用） |
| 音声パラメータ | 速度・ピッチ・抑揚（temperature）・音量 |
| プリセット | 言語ごとのプリセット（必須）。任意参照WAVを個別指定可能 |
| 辞書 | 言語ごとのクライアント読み替え辞書 |

#### Voiceger 参照WAVを任意指定する場合の注意

- 参照WAVの話者/録音条件が大きく異なると、声質が不安定になることがあります。
- 参照テキストと音声内容が一致しない場合、音質や発音が崩れやすくなります。
- 極端に短い/ノイズの多いWAVは品質低下の原因になります（目安: 数秒以上・クリア音声）。

### Audio

| 項目 | 説明 |
|---|---|
| オーディオモニタリング | 合成音声を自分でも聴く |
| マイクソース | パススルー時のマイクデバイス |
| ノイズキャンセル | RNNoise による実マイクのノイズ除去 |
| 仮想デバイス名 | PulseAudio シンク名（デフォルト: `ZunduxMic`）|
| 音声エフェクト | エコー（ディレイ ms・減衰）|
| サウンドボード | フォルダパス・音量ゲイン調整 |

## トラブルシューティング

### VOICEVOX に接続できない

```bash
curl http://127.0.0.1:50021/version
docker ps  # Docker の場合
```

設定タブで URL とポートを確認し、「VOICEVOXを再起動」ボタンを試してください。

### Voiceger で 400 エラーが出る

`tail -f /tmp/zundux_voiceger.log` でサーバーログを確認します。よくある原因:

- **サーバー起動中**: 起動完了まで数十秒かかります。しばらく待ってから再試行してください
- **中国語で G2PWModel エラー**: `GPT_SoVITS/text/G2PWModel/` にモデルが必要です（install.sh で自動ダウンロード）
- **CUDA OOM**: 他のプロセスが GPU メモリを占有しています。`nvidia-smi` で確認し、不要なプロセスを終了してください

### GPU メモリ不足 (CUDA OOM)

`tts_infer.yaml` で `is_half: true`（FP16）になっているか確認してください:

```bash
cat ~/voiceger_v2/GPT-SoVITS/GPT_SoVITS/configs/tts_infer.yaml | grep "device\|is_half"
```

FP16 でも足りない場合は `device: cpu` に変更して CPU モードに切り替えてください。

### VRChat でマイクとして認識されない

VRChat のマイク設定で **`ZunduxMic_mic`** を選択してください。

アプリ再起動後に音が出なくなった場合は、VRChat でマイクを一度別のデバイスに変えてから戻してください（PipeWire のセッションがリセットされた場合のみ必要）。

### 仮想デバイスが作成できない

```bash
pactl info  # PulseAudio / PipeWire 動作確認
```

PipeWire 環境では `pipewire-pulse` が必要です。

### 日本語が表示されない

Noto Sans CJK フォントをインストールしてください:

```bash
# Arch / Manjaro
sudo pacman -S noto-fonts-cjk
```

## ライセンス

VOICEVOX・Voiceger の音声を使用する際はそれぞれの利用規約に従ってください。各音声キャラクターにも個別の利用規約があります。
