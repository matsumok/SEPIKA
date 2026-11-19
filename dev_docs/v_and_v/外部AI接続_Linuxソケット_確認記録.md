# 外部AI接続のLinuxソケット確認記録

状態: 🔶。LinuxのUnixソケットによるGUI接続について、製品Rustの実通信の自動回帰と、WSL2（WSLg）上のGUIとサンドボックス内のMCP中継の実接続を確認した。通信と接続先の保護の有限なVerificationであり、構造計算式・規準適合の新たな認証ではない。判断は[ADR0118](../adr/0118-gui-ai-connection-unix-socket.md)。

## 対象と環境

2026-11-19、upstream main `e8367de9` から分岐した作業ブランチで確認した。Debian（trixie）のWSL2、rustc 1.99.1。テストはサンドボックス（Anthropic sandbox-runtime）内で実行し、Unixソケットの作成を許可した。サンドボックスが本来の `XDG_RUNTIME_DIR` への書き込みを許可しないため、テストでは本人専用（0700）の別ディレクトリを `XDG_RUNTIME_DIR` に指定した。ソケットパスの長さ上限（`sun_path`）に収めるため `TMPDIR=/tmp` とした。

## 自動回帰

| 検証 | コマンド要部 | 結果 |
| --- | --- | --- |
| 共有通信 | `cargo test -p sepika-ai-connection --locked` | 6成功（Unix接続先4件を含む） |
| MCP stdio | `cargo test -p sepika-mcp --features mcp --test gui_bridge_stdio --locked` | 正常中継・不一致応答の拒否の2成功 |
| AI契約 | `cargo test -p sepika-app --lib --features gui --locked app::study::ai::operations::tests -- --test-threads=1` | 28成功。実ソケットで比較案の編集・計算・提出を含む |
| 全体モデルの実通信 | `cargo test -p sepika-app --features gui --test full_model wire --locked` | 1成功 |
| 静的解析 | `cargo clippy -p sepika-ai-connection -p sepika-mcp -p sepika-app --features sepika-mcp/mcp,sepika-app/gui --all-targets --locked -- -D warnings` | Linux向け・`--target x86_64-pc-windows-gnu` とも成功 |

Unix接続先の単体テストでは、同じUIDの往復とEOF、同名パスの排他作成、受付後と未接続破棄後の接続先の不在、本人専用でないディレクトリの拒否、GUIが生成する形式以外のパスの拒否を確認した。Windows向けはクロスの静的解析だけで、Windows上のテスト実行はCIのWindowsジョブに委ねる。

## 実接続

`ai_connection_fixture` をサンドボックス外のWSLgで起動し、「ローカル接続を有効化」で `/run/user/1000/sepika-gui-<UUID>.sock` が表示された。サンドボックス内から `sepika-mcp --gui-pipe <パス>` を起動してMCPの初期化を行い、公開ツールが `gui_request` だけであることを確認した。接続確認には `awaiting_gui_approval` が返り、GUIで確認のみ（Review）を承認した後の接続確認で、接続セッション・プロジェクト・検討・基準案の入力版と `mode: Review` が返った。受付の時点で接続先のソケットファイルが削除されていた。

## 未確認

- 別UIDのプロセスからの接続の拒否（双方のUID確認はコードの確認のみ）
- 実接続での依頼開始・比較案の編集・計算・提出（同じ処理は自動回帰の実ソケットテストで確認）
- WSL2以外のLinux、macOS
- WSLgのGUIの表示・ファイルダイアログ・フォントの網羅的な確認
