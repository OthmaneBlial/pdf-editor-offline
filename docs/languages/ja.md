# PDF Editor Offline

**Rust で作るオフライン PDF エディター。ファイルは自分のパソコンに保存されます。**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

PDF を開き、必要なページを探し、文書を整理して新しいコピーを保存できます。PDF Editor Offline は無料のオープンソースのデスクトップアプリとコマンドラインツールです。アカウント、サブスクリプション、文書のアップロードは不要です。

現在はネイティブ Rust 版のプレビューです。デスクトップアプリはソースからビルドして試してください。対象は macOS、Windows、Linux で、実際のデスクトップ操作の検証は主に macOS で行っています。デスクトップ用インストーラーと完全な PDF 編集機能は開発中です。[ネイティブ CLI のプレリリース 4.0.0-preview.1](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.1)は公開済みです。[過去のデスクトップリリース](https://github.com/OthmaneBlial/pdf-editor-offline/releases)は以前のビルドです。

![macOS 上で動く実際のネイティブアプリ。ローカル検索、PDF ページ、文書情報を表示](../../screenshots/native/editorial-light-2026-10-12.png)

## 現在使える機能

| 作業 | 対応内容 |
| --- | --- |
| 閲覧と検索 | タブ、ズーム、表示中のページのサムネイル、テキスト表示、文字列検索、明暗テーマ。CLI では情報取得、テキスト抽出、限定的なテキスト・形状比較。 |
| ページ整理 | アプリではページ移動、順序反転、回転、空白ページ追加、切り抜き、元に戻す・やり直す。CLI では制限付きの抽出・削除と単純なカタログ構造の結合。 |
| 文書プロパティ | Unicode に対応した標準 6 項目の読み取り・編集：Title、Author、Subject、Keywords、Creator、Producer。 |
| 画像とコメント | 対応する PNG/JPEG から PDF ページを作成。CLI では 1 ページを PNG に出力し、Text 注釈の追加・一覧・制限付き編集・削除。 |
| 確認と保存 | CLI でアクション、添付、フォーム、レイヤーのマーカーを読み取り専用で確認。ストリームの可逆圧縮と検証済みの新規コピー保存。 |

結合、ページの部分選択、注釈編集、画像変換には明確な制限があり、対応できない構造は拒否します。[CLI ガイド](../../apps/cli/README.md)を確認してください。圧縮してもサイズが小さくなるとは限りません。切り抜きは内容を隠すだけです。メタデータや注釈を削除しても古い値が残る場合があり、安全な消去ではありません。OCR、テキスト・選択フィールドへの入力、デジタル署名、自由な本文編集、汎用文書変換は未実装です。未対応の内容は描画されないことがあるため、出力した文書を確認してください。署名済み、または正常に復号した入力は読み取り専用です。

現在のソースでは AcroForm フィールドを検出し、既存の外観を保ったまま対応するチェックボックスとラジオボタンを変更できます。[使用例と制限](../FORMS_RUST.md)。公開済み npm パッケージにはまだ含まれていません。

現在の UI は英語です。この文書の翻訳は GUI の日本語化や右から左への完全なレイアウト対応を意味しません。オフラインフォントには検証済みの中国語・日本語・韓国語の字形が含まれますが、すべての文字体系を網羅するものではありません。

## ネイティブ CLI のインストール（任意）

npm によるインストールとランチャーには Node.js 22 以降が必要です。単体の CLI 実行ファイルには Node.js は不要です。デスクトップアプリは引き続き Rust のソースからビルドしてください。

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

### 自分の PDF で試す

`input.pdf` をローカルのファイルに置き換えてください。最初のコマンドは情報を読み取り、次は書き込まずに回転を検証し、最後は新しいコピーを作成します。出力には、まだ存在しないファイル名を指定してください。

```sh
pdf-editor-offline info input.pdf --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --json
```

## ビルドと起動

Rust 1.95 以降と各 OS のネイティブビルド用ライブラリをインストールしてください。依存関係を初めてダウンロードする際は接続が必要です。Linux のパッケージは[ネイティブ CI](../../.github/workflows/rust.yml)を参照してください。

```sh
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
cargo build --release --locked -p pdf-editor-offline-native -p pdf-editor-offline-cli
./target/release/pdf-editor-offline-native
```

```sh
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --json
./target/release/pdf-editor-offline capabilities --json
```

Windows では `target\release\pdf-editor-offline-native.exe` を実行します。ページ番号は 1 から始まります。`--dry-run` は検証のみで書き込みません。既存のファイルやシンボリックリンクを上書きしないため、新しい出力先を選んでください。

## ローカル処理と検証

PDF の処理は手元のパソコンで行い、遠隔の描画・変換サービスは使いません。ただし、すべての PDF の安全性や隠れた情報の完全な除去を証明するものではありません。[CLI の検証記録](../RUST_BENCHMARKS.md)には qpdf/Poppler を用いた 44 件の操作チェック、実際のプロセス実行時間、ハッシュがあり、生成した 1,000 ページの PDF も含まれます。文書全般の再現性、デスクトップの応答性、普遍的な速度向上を示す結果ではありません。

合成 PDF による再現ケース、描画の比較、アクセシビリティテスト、翻訳の修正、小さな Rust の変更を歓迎します。公開報告には個人の文書を添付しないでください。

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

ローカルで使える PDF エディターを応援したい方は、リポジトリにスターを付けて見つけやすくしてください。

MIT ライセンス。フォントや依存関係にはそれぞれのライセンス表記があります。
