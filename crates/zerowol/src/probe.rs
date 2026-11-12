use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::net::TcpStream;

/// True if a TCP connection to `ip:port` succeeds within `timeout`.
pub async fn is_open(ip: Ipv4Addr, port: u16, timeout: Duration) -> bool {
    matches!(
        tokio::time::timeout(timeout, TcpStream::connect((ip, port))).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    const LO: Ipv4Addr = Ipv4Addr::LOCALHOST;
    const T: Duration = Duration::from_millis(500);

    #[tokio::test]
    async fn open_port_is_open() {
        let l = TcpListener::bind((LO, 0)).await.unwrap();
        assert!(is_open(LO, l.local_addr().unwrap().port(), T).await);
    }

    #[tokio::test]
    async fn closed_port_is_not_open() {
        let l = TcpListener::bind((LO, 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        assert!(!is_open(LO, port, T).await);
    }
}
