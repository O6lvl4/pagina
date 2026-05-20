<!-- description: SVG をラスタライズせずベクター PDF パスとして出力 -->
# SVG Vector Output

現在は resvg でラスタライズして画像として埋め込み。
印刷品質にはベクター出力が必要。

## Scope

- SVG パス → PDF パスへの変換
- SVG テキスト → PDF テキスト
- SVG グラデーション → PDF シェーディング
- SVG クリッピング → PDF クリッピングパス
- 外部 SVG ファイル + インライン `<svg>` 両対応

## Options

- `svg2pdf` crate（SVG → PDF 直接変換）
- usvg → 自前の PDF パス変換
- resvg でのラスタライズをフォールバックとして残す
