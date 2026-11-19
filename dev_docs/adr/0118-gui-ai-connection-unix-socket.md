# LinuxではGUIの検討操作を本人専用ランタイムディレクトリのUnixソケットで中継する

Status: proposed

## 判断

[ADR0093](0093-gui-ai-connection-protocol.md)の搬送層をLinuxへ広げる。`sepika-ai-connection`の接続先（endpoint）だけをOSごとに分け、WindowsはADR0093の限定DACL付き名前付きパイプのまま、UnixはUnixドメインソケットとする。最大1MiBのJSON行、要求・応答の搬送、GUIの許可・検討操作・受理台帳、MCP中継の応答ID照合はOSによらず共通にする。GUIとMCP中継のOS別条件分岐は残さない。

接続先は`$XDG_RUNTIME_DIR/sepika-gui-<UUID>.sock`とする。有効化の時点で、そのディレクトリが絶対パスで、実効UIDの所有で、グループ・その他に権限がないことを確認し、満たさなければ有効化を拒否する。`XDG_RUNTIME_DIR`が無い場合に`/tmp`などの共有ディレクトリへ退避しない。他ユーザーが書ける場所では、先回りのbindや接続先の差し替えを防げないためである。

既存のパスがあればbindを失敗させ、既存ファイルを削除・上書きしない。ADR0093の同名再bindを採らない判断と同じである。待受側は相手の実効UIDを`SO_PEERCRED`（Tokioの`peer_cred`）で確認し、一致する接続だけを受け付け、一致しない接続とUIDを確認できない接続は閉じて待受を続ける。接続側も待受側の実効UIDが自分と一致することを確認する。1回の有効化で受け付けるのは一つのクライアント接続だけとし、受け付けた時点で待受を閉じて接続先のパスを削除する。未接続のまま解除・終了した場合も削除する。切断後は、ADR0093と同じく人が再有効化して新しい接続先を渡す。

MCP中継の起動引数は`--gui-pipe <接続先>`のまま変えない。OSによって引数名を変えると、GUIがコピーする起動引数とクライアント設定の手順が分かれるためである。中継が受け付けるのは、GUIが生成する形式の接続先だけとする。Linuxでは`sepika-gui-`で始まり`.sock`で終わる名前の絶対パスで、`..`を含まないものに限る。

TCPの待受は採らない。ポートの探索や競合が生じ、同じ端末の別ユーザーからの接続をOSの権限で区別できないためである。Linuxの抽象名前空間ソケットも採らない。ファイルの権限で保護できず、ネットワーク名前空間を分けるサンドボックスからは到達できないためである。

## 確認と限界

- 本人限定の単位はUnixの実効UIDである。WindowsのログオンSIDより広く、同じユーザーの別ログインセッションも接続できる。同じユーザーの悪意あるプロセス全般を隔離しない点は[ADR0087](0087-gui-owned-study-and-local-ai.md)と同じである。
- 別UIDからの接続の拒否は、コードの確認だけで実測していない。
- macOSは`cfg(unix)`でビルド対象に入るが、検証していない。
- サンドボックス内のクライアントから接続するには、Unixソケットの作成と接続先パスの読み取りを許可する必要がある。
- WSLgでGUIを使う場合の表示・ファイルダイアログ・フォントの確認は、本判断の範囲外とする。
- 実測の範囲は[Linux接続確認記録](../v_and_v/外部AI接続_Linuxソケット_確認記録.md)で区別する。

## 参照

- [unix(7)：SO_PEERCRED](https://man7.org/linux/man-pages/man7/unix.7.html)
- [XDG Base Directory Specification：XDG_RUNTIME_DIR](https://specifications.freedesktop.org/basedir-spec/latest/)
- [Tokio UnixStream::peer_cred](https://docs.rs/tokio/latest/tokio/net/struct.UnixStream.html#method.peer_cred)
