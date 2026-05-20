<!-- description: html5ever + cssparser + printpdf パイプライン構築、@page size/margin → PDF -->
<!-- done: 2026-05-20 -->
# Initial Pipeline

html5ever (HTML) + cssparser (CSS) + printpdf (PDF) のパイプライン構築。

- HTML パース → DOM tree
- CSS パース → @page { size, margin }
- テキスト抽出 → 行折り返し → ページ分割
- PDF 出力（Helvetica ビルトインフォント）
