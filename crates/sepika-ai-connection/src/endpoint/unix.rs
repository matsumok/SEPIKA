use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use tokio::net::{UnixListener, UnixStream};

const PREFIX: &str = "sepika-gui-";
const SUFFIX: &str = ".sock";

pub type Client = UnixStream;

fn effective_uid() -> u32 {
    unsafe { libc::geteuid() }
}

/// `XDG_RUNTIME_DIR` に有効化ごとに異なるランダムなソケットパスを返します。
/// 未設定、または本人所有で他者に権限のないディレクトリでなければ失敗します。
pub fn new_name() -> io::Result<String> {
    // /tmpへ退避しないのは、他ユーザーも書ける場所では接続先の差し替えを防げないため。
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| io::Error::other("XDG_RUNTIME_DIRが設定されていません"))?;
    name_in(Path::new(&dir))
}

fn name_in(dir: &Path) -> io::Result<String> {
    let metadata = std::fs::metadata(dir)?;
    if !dir.is_absolute()
        || !metadata.is_dir()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::other(
            "XDG_RUNTIME_DIRが本人専用のディレクトリではありません",
        ));
    }
    dir.join(format!("{PREFIX}{}{SUFFIX}", uuid::Uuid::new_v4()))
        .into_os_string()
        .into_string()
        .map_err(|_| io::Error::other("接続先のパスがUTF-8ではありません"))
}

/// GUIが生成する形式のソケットの絶対パスであれば真を返します。
pub fn is_gui_endpoint(name: &str) -> bool {
    let path = Path::new(name);
    path.is_absolute()
        && !path.components().any(|c| c == Component::ParentDir)
        && path
            .file_name()
            .and_then(|f| f.to_str())
            .and_then(|f| f.strip_prefix(PREFIX))
            .and_then(|f| f.strip_suffix(SUFFIX))
            .is_some_and(|id| !id.is_empty())
}

/// 待受ソケットと接続先パスを所有し、破棄時にパスを削除します。
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 同じパスが既にあれば失敗し、既存のファイルを置き換えません。
pub fn create(name: &str) -> io::Result<Server> {
    Ok(Server {
        listener: UnixListener::bind(name)?,
        path: name.into(),
    })
}

/// 同じ実効UIDのクライアント接続を一つだけ受け付けて返し、接続先を削除します。
/// 別UIDまたはUIDを確認できない接続は閉じて待受を続けます。
pub async fn accept(server: Server) -> io::Result<UnixStream> {
    loop {
        let (stream, _) = server.listener.accept().await?;
        if stream
            .peer_cred()
            .is_ok_and(|peer| peer.uid() == effective_uid())
        {
            return Ok(stream);
        }
    }
}

/// 指定GUIへ接続します。待受側が同じ実効UIDのプロセスでなければ失敗します。
pub fn connect(name: &str) -> io::Result<UnixStream> {
    let stream = std::os::unix::net::UnixStream::connect(name)?;
    stream.set_nonblocking(true)?;
    let stream = UnixStream::from_std(stream)?;
    if stream.peer_cred()?.uid() != effective_uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "接続先が別ユーザーのプロセスです",
        ));
    }
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn private_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sepika-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    #[tokio::test]
    async fn same_user_round_trip_eof_missing_and_exclusive_creation() {
        let dir = private_dir();
        let name = name_in(&dir).unwrap();
        let server = create(&name).unwrap();
        assert!(create(&name).is_err());
        let mut client = connect(&name).unwrap();
        let mut stream = accept(server).await.unwrap();
        assert!(connect(&name).is_err());
        client.write_all("検討\n".as_bytes()).await.unwrap();
        let mut buffer = [0; 7];
        stream.read_exact(&mut buffer).await.unwrap();
        assert_eq!(&buffer, "検討\n".as_bytes());
        stream.write_all(b"accepted\n").await.unwrap();
        let mut response = [0; 9];
        client.read_exact(&mut response).await.unwrap();
        assert_eq!(&response, b"accepted\n");
        drop(client);
        assert_eq!(stream.read(&mut buffer).await.unwrap(), 0);
        std::fs::remove_dir(&dir).unwrap();
    }

    #[tokio::test]
    async fn dropping_an_unaccepted_server_retires_the_endpoint() {
        let dir = private_dir();
        let name = name_in(&dir).unwrap();
        drop(create(&name).unwrap());
        assert!(connect(&name).is_err());
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn runtime_directory_must_be_private_to_the_user() {
        let dir = private_dir();
        let name = name_in(&dir).unwrap();
        assert!(is_gui_endpoint(&name));
        assert_ne!(name, name_in(&dir).unwrap());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(name_in(&dir).is_err());
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn only_absolute_sepika_socket_paths_are_gui_endpoints() {
        assert!(is_gui_endpoint("/run/user/1000/sepika-gui-a.sock"));
        assert!(!is_gui_endpoint("sepika-gui-a.sock"));
        assert!(!is_gui_endpoint(
            "/run/user/1000/../../tmp/sepika-gui-a.sock"
        ));
        assert!(!is_gui_endpoint("/run/user/1000/sepika-gui-.sock"));
        assert!(!is_gui_endpoint("/run/user/1000/other-a.sock"));
        assert!(!is_gui_endpoint(r"\\.\pipe\sepika-gui-a"));
    }
}
