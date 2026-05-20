<!-- description: テーブル自動列幅、セル結合、ページ跨ぎ、border-collapse -->
# Table Auto-Width Layout

現在はテーブル列が等幅分割。実用的なテーブルには自動列幅計算が必要。

## Scope

- 列幅の自動計算（コンテンツ幅に基づく）
- colspan / rowspan セル結合
- ページ跨ぎ時のヘッダ繰り返し（thead の自動リピート）
- border-collapse: collapse / separate
- セル内テキストの折り返し
- 明示的な列幅指定（width CSS プロパティ）

## Implementation

- layout.rs の `lay_out_table()` を 2 パス化:
  1. 全セルを走査して最小幅・推奨幅を計算
  2. 利用可能幅に基づいて列幅を配分
- CSS Table Module Level 3 の auto layout algorithm を参考に
