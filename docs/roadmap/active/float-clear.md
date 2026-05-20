<!-- description: CSS float / clear によるテキスト回り込み -->
# Float / Clear

画像やサイドバーの横にテキストを回り込ませる CSS float。

## Scope

- `float: left` / `float: right`
- `clear: left` / `clear: right` / `clear: both`
- float 要素の周囲のテキスト回り込みレイアウト
- ページ跨ぎ時の float 処理

## Implementation

- layout.rs にフロートコンテキストを追加
- 行ごとに利用可能幅を float 要素の幅分だけ縮小
- clear で float をクリアしてフル幅に戻す
