<!-- description: PDF 内部/外部リンク（<a href> → PDF Link Annotation） -->
# PDF Links

HTML の `<a href>` をクリック可能な PDF リンクに変換。

## Scope

- 外部リンク: `<a href="https://...">` → URI アクション
- 内部リンク: `<a href="#section">` → ページ内ジャンプ（GoTo アクション）
- リンクの矩形領域計算
- リンクの視覚スタイル（下線、色）

## Implementation

- layout.rs でリンク要素の位置・サイズを記録
- pdf.rs で printpdf の `LinkAnnotation` Op を使用
