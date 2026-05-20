<!-- description: break-inside: avoid（要素途中での改ページ抑制） -->
# break-inside: avoid

テーブル行、図表、コードブロックなどが途中で改ページされないようにする。

## Scope

- `break-inside: avoid` プロパティ
- `page-break-inside: avoid`（レガシー互換）
- 対象要素のレイアウト前に必要高さを見積もり
- 収まらない場合は改ページ後に要素全体を配置
- orphans / widows プロパティ（段落の最小行数制御）

## Implementation

- layout.rs の lay_out_block() で、break-inside: avoid の要素は
  事前に全行の高さを計算し、残りスペースに収まるか判定
- 収まらなければ new_page() してから配置
