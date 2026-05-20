<!-- description: WASM ビルドでブラウザ内 PDF 生成 -->
# WASM Target

Rust 製の強みを活かしてブラウザ内で PDF 生成できるようにする。

## Scope

- `wasm32-unknown-unknown` ターゲットでのビルド
- JavaScript/TypeScript バインディング（wasm-bindgen）
- npm パッケージとして公開
- ファイルシステム非依存の API（バイト列入出力）
- ストリーミング出力（大きな文書のメモリ効率）

## Blockers

- resvg: WASM 対応が必要（tiny-skia は対応済み）
- boa_engine: WASM 対応が必要（または JS ランタイムを省略）
- image crate: WASM 対応（feature flag で制御）
- ファイルパスを使う機能（画像読み込み等）の抽象化

## Implementation

- `pagina-core` の IO 依存をトレイトに抽象化
- `pagina-wasm` クレートを追加
- wasm-pack でビルド & npm 公開
