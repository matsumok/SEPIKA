//! GUIとMCP中継が使うOSごとの使い捨て接続先。Windowsは名前付きパイプ、UnixはUnixドメインソケット。

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;
