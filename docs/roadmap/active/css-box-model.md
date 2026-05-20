<!-- description: 完全な CSS ボックスモデル（width/height, box-sizing, overflow） -->
# CSS Box Model

現在は margin/padding のみ。完全なボックスモデルの実装。

## Scope

- `width` / `height` / `min-width` / `max-width` / `min-height` / `max-height`
- `box-sizing: content-box` / `border-box`
- `overflow: hidden` / `visible`
- `border` (全辺、個別辺、border-radius は Phase 3)
- `background-color`
- マージン相殺（margin collapsing）

## Implementation

- layout.rs のブロックレイアウトを拡張
- 各ブロック要素に LayoutBox（content + padding + border + margin）を生成
- pdf.rs にボックス背景・ボーダーの描画を追加
