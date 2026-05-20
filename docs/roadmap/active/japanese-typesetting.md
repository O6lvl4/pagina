<!-- description: 日本語組版（禁則処理、縦書き、ルビ） -->
# Japanese Typesetting

日本語文書の組版対応。印刷品質の日本語 PDF 生成。

## Scope

- 禁則処理（行頭禁則・行末禁則文字）
- CJK 文字幅計算（全角/半角判定）
- 縦書き（`writing-mode: vertical-rl`）
- ルビ（`<ruby>`, `<rt>`）
- 約物の半角化（句読点の前後スペース調整）
- `text-spacing` プロパティ（JIS X 4051 準拠）

## Dependencies

- CJK 対応フォント埋め込み（@font-face 依存）
- Unicode East Asian Width プロパティ

## Implementation

- layout.rs の行分割に CJK 改行ルールを追加
- 縦書きは座標変換（x/y 入れ替え + 回転）で実現
- ルビは行間にオーバーラインテキストとして配置
