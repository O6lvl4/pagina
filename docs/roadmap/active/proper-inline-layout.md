<!-- description: インラインフォーマッティングコンテキスト、ベースライン揃え、bidi -->
# Proper Inline Formatting Context

現在のインラインレイアウトは単語単位のスタイル切り替えのみ。
正確なインラインレイアウトにはベースライン揃えと行ボックスの概念が必要。

## Scope

- Line box モデル（CSS 2.1 Section 9.4.2）
- ベースライン揃え（異なるフォントサイズの混在時）
- `vertical-align: top` / `middle` / `bottom` / `baseline` / `super` / `sub`
- インラインブロック（`display: inline-block`）
- Bidirectional text（Unicode Bidi Algorithm、RTL 言語対応）

## Implementation

- layout.rs に LineBox 構造体を導入
- 各 InlineSegment にベースライン位置を計算
- 行ボックスの高さ = 最大アセンダー + 最大ディセンダー
