<!-- description: 子孫・子・隣接・属性セレクタ、疑似クラス、::before/::after -->
# Descendant & Advanced Selectors

現在はタグ/クラス/ID の単純セレクタのみ。実用的な CSS には子孫セレクタが必須。

## Scope

- 子孫セレクタ: `.toc a`, `table td`
- 子セレクタ: `ul > li`
- 隣接セレクタ: `h2 + p`
- 属性セレクタ: `a[href]`, `input[type="text"]`
- 疑似クラス: `:first-child`, `:last-child`, `:nth-child()`
- 疑似要素: `::before`, `::after` (content プロパティと連携)

## Why

target-counter の目次で `.toc a { content: ... }` が使えないのはこれが原因。
::after 疑似要素は Prince の目次パターンの標準的な書き方。

## Implementation

style.rs の `selector_matches()` を拡張:
- セレクタを「コンビネータで区切られたシンプルセレクタの列」として解析
- マッチ時に DOM ツリーを遡って祖先を検査
- 疑似要素は styled tree にダミーノードとして挿入
