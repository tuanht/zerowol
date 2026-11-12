use std::net::Ipv4Addr;

pub fn magic_packet(mac: [u8; 6]) -> [u8; 102] {
    let mut p = [0xFFu8; 102];
    for i in 0..16 {
        p[6 + i * 6..12 + i * 6].copy_from_slice(&mac);
    }
    p
}

pub async fn send(mac: [u8; 6], broadcast: Ipv4Addr, port: u16) -> std::io::Result<()> {
    let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
    sock.set_broadcast(true)?;
    sock.send_to(&magic_packet(mac), (broadcast, port)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn packet_layout() {
        let mac = [1, 2, 3, 4, 5, 6];
        let p = magic_packet(mac);
        assert_eq!(p.len(), 102);
        assert_eq!(p[0..6], [0xFF; 6]);
        assert_eq!(p[6..12], mac);
        assert_eq!(p[96..102], mac);
    }

    #[tokio::test]
    async fn send_delivers_packet() {
        let rx = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = rx.local_addr().unwrap().port();
        let mac = [0xD8, 0x5E, 0xD3, 0xDB, 0xF7, 0x6B];
        send(mac, Ipv4Addr::LOCALHOST, port).await.unwrap();
        let mut buf = [0u8; 256];
        let n = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv(&mut buf))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&buf[..n], &magic_packet(mac)[..]);
    }
}
