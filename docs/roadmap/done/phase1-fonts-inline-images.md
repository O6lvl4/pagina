<!-- description: ttf-parser グリフ幅、インライン混在スタイル、外部フォント埋め込み、画像 -->
<!-- done: 2026-05-20 -->
# Phase 1: Fonts, Inline Styles, Images

- ttf-parser による正確なグリフ幅計測（ビルトイン 14 フォント全対応）
- インライン混在スタイル（bold/italic/color が行内で切り替わる）
- 外部フォント読み込み（TTF/OTF、--font CLI フラグ）
- 画像埋め込み（PNG/JPEG、<img src="...">）
- 同スタイルセグメントのマージ（PDF テキスト抽出でスペース保持）
