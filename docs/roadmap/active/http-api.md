<!-- description: HTTP API サーバーモード（Docker / クラウド展開用） -->
# HTTP API

サーバーモードで HTTP API を提供し、リモートから PDF 生成リクエストを受ける。

## Scope

- `pagina serve --port 8080` サブコマンド
- `POST /convert` エンドポイント（HTML body → PDF response）
- マルチパートフォームでのフォント・画像アップロード
- ヘルスチェック / メトリクス エンドポイント
- Docker イメージの公式提供
- 同時リクエスト処理（Tokio ベース非同期）

## Implementation

- `pagina-server` クレートを追加
- axum または actix-web フレームワーク
- Dockerfile + docker-compose.yml
