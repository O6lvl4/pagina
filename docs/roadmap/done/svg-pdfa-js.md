<!-- description: SVG (resvg)、PDF/A (XMP + OutputIntent)、JavaScript (Boa engine) -->
<!-- done: 2026-05-20 -->
# SVG, PDF/A, JavaScript

- SVG 埋め込み: resvg で 300DPI ラスタライズ → 画像として埋め込み
- PDF/A-1b: XMP メタデータ + OutputIntent (sRGB) を lopdf で注入
- JavaScript 実行: Boa engine で <script> ブロックを実行、document.write() でコンテンツ生成
