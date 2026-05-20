<!-- description: @font-face CSS ルール、フォントサブセット、WOFF/WOFF2 -->
# @font-face

現在は --font CLI フラグでのみ外部フォント読み込み可能。
CSS の @font-face でフォントを宣言できるようにする。

## Scope

- `@font-face { font-family: "..."; src: url("..."); }` パース
- `font-weight` / `font-style` ディスクリプタ
- WOFF / WOFF2 形式の解凍
- フォントサブセット（使用文字のみ埋め込み、PDF サイズ削減）
- CSS `font-family` フォールバックチェーン

## Implementation

- css/parser.rs に @font-face パーサー追加
- font.rs にフォントフォールバック解決ロジック
- printpdf の subset_font() を活用
