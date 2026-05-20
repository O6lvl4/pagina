<!-- description: PDF ブックマーク（しおり）自動生成 -->
# PDF Bookmarks

見出し要素（h1-h6）から PDF ブックマーク（しおり/アウトライン）を自動生成。

## Scope

- h1-h6 から階層的なブックマークツリーを構築
- 各ブックマークからページ内位置へのリンク
- CSS `bookmark-level` / `bookmark-label` プロパティ（GCPM）
- printpdf の `add_bookmark()` API を使用

## Implementation

- layout.rs でブックマーク情報を収集（テキスト + ページ番号 + Y 位置）
- pdf.rs で `doc.add_bookmark()` を呼び出し
