<!-- description: ハイフネーション（英語・日本語の自動分割） -->
# Hyphenation

長い単語が行末で収まらない場合にハイフンで分割する。

## Scope

- CSS `hyphens: auto` プロパティ
- 英語ハイフネーション（Knuth-Liang アルゴリズム）
- ハイフネーション辞書の埋め込み（hypher / hyphenation crate）
- 日本語禁則処理（行頭禁則・行末禁則）
- `word-break: break-all` / `overflow-wrap: break-word`

## Dependencies

- `hyphenation` crate（多言語ハイフネーションパターン）

## Implementation

- layout.rs の `break_into_lines()` を拡張
- 行末に収まらない単語をハイフネーションポイントで分割
- 日本語は文字単位の改行（禁則文字チェック付き）
