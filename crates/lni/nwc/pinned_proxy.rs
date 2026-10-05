//! Restricted, process-local SOCKS5 dial gate for upstream NWC, whose relay pool
//! does not expose custom transports. WSS/TLS still uses the original relay URL.
//! Every reconnect resolves afresh, rejects the entire answer set if unsafe, and
//! connects only to validated concrete SocketAddrs. No second DNS lookup occurs.
use std::{
    future::Future,
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_SESSIONS: usize = 8;

#[derive(Clone, PartialEq, Eq)]
struct Target {
    host: String,
    port: u16,
}

pub(super) struct PinnedProxy {
    pub address: SocketAddr,
    task: JoinHandle<()>,
}
impl Drop for PinnedProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn denied() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "relay dial denied")
}

fn target(value: &str) -> io::Result<Target> {
    let url = reqwest::Url::parse(value).map_err(|_| denied())?;
    if url.scheme() != "wss"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(denied());
    }
    let host = url
        .host_str()
        .ok_or_else(denied)?
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    let port = url.port_or_known_default().ok_or_else(denied)?;
    if port == 0 || host.is_empty() {
        return Err(denied());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        checked_addresses(&[SocketAddr::new(ip, port)], port)?;
    }
    Ok(Target { host, port })
}

fn checked_addresses(addresses: &[SocketAddr], port: u16) -> io::Result<()> {
    if addresses.is_empty()
        || addresses.len() > 64
        || addresses
            .iter()
            .any(|address| address.port() != port || !crate::lnurl::is_public_ip(address.ip()))
    {
        return Err(denied());
    }
    Ok(())
}

async fn system_resolve(host: String, port: u16) -> io::Result<Vec<SocketAddr>> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    Ok(tokio::net::lookup_host((host.as_str(), port))
        .await?
        .take(65)
        .collect())
}

impl PinnedProxy {
    pub async fn start(relays: impl Iterator<Item = impl AsRef<str>>) -> io::Result<Self> {
        let allowed: Vec<Target> = relays
            .map(|relay| target(relay.as_ref()))
            .collect::<io::Result<_>>()?;
        if allowed.is_empty() || allowed.len() > 8 {
            return Err(denied());
        }
        let allowed = Arc::new(allowed);
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let task = tokio::spawn(async move {
            // Aborting this parent drops JoinSet, closing all admitted sessions.
            let mut sessions = JoinSet::new();
            loop {
                tokio::select! {
                    _ = sessions.join_next(), if !sessions.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((stream,_)) = accepted else { break; };
                        if sessions.len() >= MAX_SESSIONS { drop(stream); continue; }
                        let allowed = allowed.clone();
                        sessions.spawn(async move { let _ = serve(stream,allowed,system_resolve,|address| TcpStream::connect(address)).await; });
                    }
                }
            }
        });
        Ok(Self { address, task })
    }
}

async fn request(stream: &mut TcpStream, allowed: &[Target]) -> io::Result<Target> {
    let mut greeting = [0; 2];
    stream.read_exact(&mut greeting).await?;
    if greeting[0] != 5 || greeting[1] == 0 || greeting[1] > 8 {
        return Err(denied());
    }
    let mut methods = vec![0; greeting[1] as usize];
    stream.read_exact(&mut methods).await?;
    if !methods.contains(&0) {
        stream.write_all(&[5, 255]).await?;
        return Err(denied());
    }
    stream.write_all(&[5, 0]).await?;
    let mut header = [0; 4];
    stream.read_exact(&mut header).await?;
    if header[..3] != [5, 1, 0] {
        return Err(denied());
    }
    let host = match header[3] {
        1 => {
            let mut ip = [0; 4];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        4 => {
            let mut ip = [0; 16];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        3 => {
            let length = stream.read_u8().await?;
            if length == 0 {
                return Err(denied());
            }
            let mut bytes = vec![0; length as usize];
            stream.read_exact(&mut bytes).await?;
            let value = std::str::from_utf8(&bytes).map_err(|_| denied())?;
            if !value.is_ascii() {
                return Err(denied());
            }
            value.trim_matches(['[', ']']).to_ascii_lowercase()
        }
        _ => return Err(denied()),
    };
    let port = stream.read_u16().await?;
    let requested = Target { host, port };
    if !allowed.contains(&requested) {
        return Err(denied());
    }
    Ok(requested)
}

async fn serve<R, RF, D, DF>(
    mut inbound: TcpStream,
    allowed: Arc<Vec<Target>>,
    resolve: R,
    dial: D,
) -> io::Result<()>
where
    R: Fn(String, u16) -> RF,
    RF: Future<Output = io::Result<Vec<SocketAddr>>>,
    D: Fn(SocketAddr) -> DF,
    DF: Future<Output = io::Result<TcpStream>>,
{
    let handshake = async {
        let requested = request(&mut inbound, &allowed).await?;
        let addresses = resolve(requested.host, requested.port).await?;
        checked_addresses(&addresses, requested.port)?;
        for address in addresses {
            // Concrete address only: the original hostname never reaches TcpStream::connect.
            if let Ok(stream) = dial(address).await {
                return Ok(stream);
            }
        }
        Err(denied())
    };
    let upstream = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await;
    let mut upstream = match upstream {
        Ok(Ok(stream)) => stream,
        _ => {
            let _ = inbound.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await;
            return Err(denied());
        }
    };
    inbound.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
    tokio::io::copy_bidirectional(&mut inbound, &mut upstream).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_dns_answers_must_be_public_and_same_port() {
        let public: SocketAddr = "93.184.216.34:443".parse().unwrap();
        assert!(checked_addresses(&[public], 443).is_ok());
        for bad in [
            "127.0.0.1:443",
            "10.0.0.1:443",
            "100.64.0.1:443",
            "[::1]:443",
            "[::ffff:127.0.0.1]:443",
            "[2001:20::1]:443",
            "[3fff::1]:443",
            "93.184.216.34:80",
        ] {
            assert!(checked_addresses(&[public, bad.parse().unwrap()], 443).is_err());
        }
        assert!(target("ws://relay.example").is_err());
        assert!(target("wss://127.0.0.1").is_err());
    }
    async fn socks(stream: &mut TcpStream, host: &str, port: u16) {
        stream.write_all(&[5, 1, 0]).await.unwrap();
        let mut method = [0; 2];
        stream.read_exact(&mut method).await.unwrap();
        assert_eq!(method, [5, 0]);
        let mut bytes = vec![5, 1, 0, 3, host.len() as u8];
        bytes.extend_from_slice(host.as_bytes());
        bytes.extend_from_slice(&port.to_be_bytes());
        stream.write_all(&bytes).await.unwrap();
    }
    #[tokio::test]
    async fn socks_forwards_to_pinned_address_and_keeps_host_out_of_dial() {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local = upstream.local_addr().unwrap();
        let echo = tokio::spawn(async move {
            let (mut s, _) = upstream.accept().await.unwrap();
            let mut bytes = [0; 4];
            s.read_exact(&mut bytes).await.unwrap();
            s.write_all(&bytes).await.unwrap();
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            serve(
                stream,
                Arc::new(vec![Target {
                    host: "relay.example".into(),
                    port: 443,
                }]),
                |host, port| async move {
                    assert_eq!(host, "relay.example");
                    assert_eq!(port, 443);
                    Ok(vec!["93.184.216.34:443".parse().unwrap()])
                },
                move |address| async move {
                    assert_eq!(address, "93.184.216.34:443".parse::<SocketAddr>().unwrap());
                    TcpStream::connect(local).await
                },
            )
            .await
            .unwrap();
        });
        let mut socket = TcpStream::connect(endpoint).await.unwrap();
        socks(&mut socket, "relay.example", 443).await;
        let mut reply = [0; 10];
        socket.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply[1], 0);
        socket.write_all(b"test").await.unwrap();
        let mut echo_bytes = [0; 4];
        socket.read_exact(&mut echo_bytes).await.unwrap();
        assert_eq!(&echo_bytes, b"test");
        drop(socket);
        server.await.unwrap();
        echo.await.unwrap();
    }
    #[tokio::test]
    async fn rejects_other_hosts_and_drop_closes_listener() {
        let proxy = PinnedProxy::start(["wss://relay.example"].into_iter())
            .await
            .unwrap();
        let endpoint = proxy.address;
        let mut socket = TcpStream::connect(endpoint).await.unwrap();
        socks(&mut socket, "127.0.0.1", 443).await;
        let mut reply = [0; 10];
        socket.read_exact(&mut reply).await.unwrap();
        assert_ne!(reply[1], 0);
        let mut idle = TcpStream::connect(endpoint).await.unwrap();
        drop(proxy);
        let result = tokio::time::timeout(Duration::from_secs(1), idle.read_u8())
            .await
            .unwrap();
        assert!(result.is_err());
        assert!(TcpStream::connect(endpoint).await.is_err());
    }
}
