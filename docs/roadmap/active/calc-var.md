<!-- description: CSS calc() 関数と var() カスタムプロパティ -->
# calc() and var()

CSS の計算関数とカスタムプロパティ。

## Scope

- `calc()`: 加減乗除、異なる単位の混在（`calc(100% - 20mm)`）
- `var()`: カスタムプロパティ（`--primary-color: navy; color: var(--primary-color);`）
- `:root` 疑似クラスでのカスタムプロパティ宣言
- calc() 内での var() 参照

## Implementation

- css/values.rs に CalcExpression 型を追加
- 値の解決を遅延化（レイアウト時にコンテキスト依存の値を計算）
- カスタムプロパティを style resolution のカスケードで伝播
