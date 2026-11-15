//! Authenticated TCP tunnels over libp2p streams.
//! Worker side: accepts streams from trusted peers and bridges them to a workload's
//! loopback port (only ports this agent deployed). Gateway side: a local TCP listener
//! that forwards each connection to the chosen worker.
use std::{collections::HashSet, net::SocketAddr, sync::{Arc, Mutex}, time::Duration};

use anyhow::{bail, Result};
use libp2p::{futures::StreamExt, PeerId, StreamProtocol};
use libp2p_stream as stream;
use std::{collections::HashMap, net::IpAddr, time::Instant};
use tokio::{io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::Semaphore};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, warn};

pub const PROTO: StreamProtocol = StreamProtocol::new("/flakenet/tunnel/1.0.0");
pub type Allowed = Arc<Mutex<HashSet<u16>>>;
const MAX_TUNNELS: usize = 64;
pub const HEALTH: StreamProtocol = StreamProtocol::new("/flakenet/health/1.0.0");
pub const IDLE: Duration = Duration::from_secs(300);

/// Bidirectional copy that ends when either side closes or no data flows for `idle`.
pub async fn proxy<A, B>(a: A, b: B, idle: Duration) -> Result<()>
where A: AsyncRead + AsyncWrite + Unpin, B: AsyncRead + AsyncWrite + Unpin {
    let (mut ar, mut aw) = tokio::io::split(a);
    let (mut br, mut bw) = tokio::io::split(b);
    async fn pipe<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(r: &mut R, w: &mut W, idle: Duration) -> Result<()> {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let n = tokio::time::timeout(idle, r.read(&mut buf)).await.map_err(|_| anyhow::anyhow!("idle timeout"))??;
            if n == 0 { let _ = w.shutdown().await; return Ok(()); }
            w.write_all(&buf[..n]).await?;
        }
    }
    tokio::select! {
        r = pipe(&mut ar, &mut bw, idle) => r,
        r = pipe(&mut br, &mut aw, idle) => r,
    }
}

/// Fixed-window per-IP connection rate limiter.
pub struct RateLimiter { max: u32, window: Duration, hits: Mutex<HashMap<IpAddr, (u32, Instant)>> }
impl RateLimiter {
    pub fn new(max: u32, window: Duration) -> Self { Self { max, window, hits: Mutex::new(HashMap::new()) } }
    pub fn allow(&self, ip: IpAddr) -> bool {
        let mut h = self.hits.lock().unwrap();
        let now = Instant::now();
        if h.len() > 10_000 { h.retain(|_, (_, t)| now.duration_since(*t) < self.window); }
        let e = h.entry(ip).or_insert((0, now));
        if now.duration_since(e.1) >= self.window { *e = (0, now); }
        e.0 += 1;
        e.0 <= self.max
    }
}

pub struct GatewayOpts { pub tls: Option<Arc<ServerConfig>>, pub rate_per_10s: u32 }

pub fn tls_config(cert_pem: &[u8], key_pem: &[u8]) -> Result<Arc<ServerConfig>> {
    let certs = rustls_pemfile::certs(&mut &*cert_pem).collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut &*key_pem)?.ok_or_else(|| anyhow::anyhow!("no private key"))?;
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    Ok(Arc::new(
        ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(certs, key)?,
    ))
}

pub fn self_signed(domain: &str) -> Result<Arc<ServerConfig>> {
    let ck = rcgen::generate_simple_self_signed(vec![domain.to_string()])?;
    tls_config(ck.cert.pem().as_bytes(), ck.signing_key.serialize_pem().as_bytes())
}

pub async fn serve(mut control: stream::Control, trusted: Vec<PeerId>, allowed: Allowed) {
    let mut incoming = match control.accept(PROTO) {
        Ok(i) => i,
        Err(e) => { warn!("tunnel accept: {e}"); return; }
    };
    let sem = Arc::new(Semaphore::new(MAX_TUNNELS));
    while let Some((peer, s)) = incoming.next().await {
        if !trusted.contains(&peer) { warn!(%peer, "TUNNEL DENIED: untrusted peer"); continue; }
        let Ok(permit) = sem.clone().try_acquire_owned() else { warn!("tunnel limit reached"); continue; };
        let allowed = allowed.clone();
        tokio::spawn(async move {
            let _p = permit;
            if let Err(e) = handle(s, allowed).await { warn!(%peer, "tunnel closed: {e:#}"); }
        });
    }
}

async fn handle(s: libp2p::Stream, allowed: Allowed) -> Result<()> {
    let mut s = s.compat();
    let mut buf = [0u8; 2];
    tokio::time::timeout(Duration::from_secs(5), tokio::io::AsyncReadExt::read_exact(&mut s, &mut buf)).await??;
    let port = u16::from_be_bytes(buf);
    if !allowed.lock().unwrap().contains(&port) { bail!("port {port} is not a deployed workload"); }
    let mut tcp = TcpStream::connect(("127.0.0.1", port)).await?;
    proxy(&mut s, &mut tcp, IDLE).await
}

pub async fn gateway(listen: SocketAddr, control: stream::Control, target: tokio::sync::watch::Receiver<(PeerId, u16)>, opts: GatewayOpts) -> Result<()> {
    let l = TcpListener::bind(listen).await?;
    let scheme = if opts.tls.is_some() { "https" } else { "http" };
    info!(%listen, "GATEWAY listening ({scheme})");
    let tunnels = Arc::new(Semaphore::new(MAX_TUNNELS)); // established tunnels
    let sem = Arc::new(Semaphore::new(1024)); // connections still handshaking (time-limited)
    let limiter = Arc::new(RateLimiter::new(opts.rate_per_10s, Duration::from_secs(10)));
    let acceptor = opts.tls.map(TlsAcceptor::from);
    loop {
        let (c, addr) = match l.accept().await { Ok(x) => x, Err(e) => { warn!("accept: {e}"); tokio::time::sleep(Duration::from_millis(50)).await; continue; } };
        if !limiter.allow(addr.ip()) { warn!(ip = %addr.ip(), "RATE LIMITED"); continue; }
        let Ok(permit) = sem.clone().try_acquire_owned() else { warn!("gateway busy"); continue; };
        let (peer, port) = *target.borrow(); // follows failover
        let (mut ctl, acceptor, tunnels) = (control.clone(), acceptor.clone(), tunnels.clone());
        tokio::spawn(async move {
            let _p = permit;
            let r: Result<()> = async {
                // Finish the (time-limited) TLS handshake BEFORE spending a worker stream.
                match acceptor {
                    Some(a) => {
                        let t = tokio::time::timeout(Duration::from_secs(10), a.accept(c)).await??;
                        let _tp = tunnels.try_acquire_owned().map_err(|_| anyhow::anyhow!("tunnel limit reached"))?;
                        let s = open(&mut ctl, peer, port).await?;
                        proxy(t, s, IDLE).await
                    }
                    None => {
                        let _tp = tunnels.try_acquire_owned().map_err(|_| anyhow::anyhow!("tunnel limit reached"))?;
                        let s = open(&mut ctl, peer, port).await?;
                        proxy(c, s, IDLE).await
                    }
                }
            }.await;
            if let Err(e) = r { warn!(%addr, "gateway connection: {e:#}"); }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_blocks_over_limit_per_ip() {
        let rl = RateLimiter::new(3, Duration::from_secs(10));
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        let b: IpAddr = "10.0.0.2".parse().unwrap();
        assert!((0..3).all(|_| rl.allow(a)));
        assert!(!rl.allow(a));
        assert!(rl.allow(b)); // other IPs unaffected
    }

    #[test]
    fn rate_limiter_window_resets() {
        let rl = RateLimiter::new(1, Duration::from_millis(30));
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(rl.allow(a));
        assert!(!rl.allow(a));
        std::thread::sleep(Duration::from_millis(40));
        assert!(rl.allow(a));
    }

    #[tokio::test]
    async fn proxy_idle_timeout_fires() {
        let (a, _a_peer) = tokio::io::duplex(64);
        let (b, _b_peer) = tokio::io::duplex(64);
        let r = proxy(a, b, Duration::from_millis(50)).await;
        assert!(r.unwrap_err().to_string().contains("idle"));
    }

    #[tokio::test]
    async fn proxy_forwards_data() {
        let (a, mut a_peer) = tokio::io::duplex(64);
        let (b, mut b_peer) = tokio::io::duplex(64);
        let t = tokio::spawn(proxy(a, b, Duration::from_secs(2)));
        a_peer.write_all(b"hello").await.unwrap();
        let mut buf = [0u8; 5];
        b_peer.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
        drop(a_peer); drop(b_peer);
        let _ = t.await;
    }

    #[test]
    fn self_signed_config_builds() { assert!(self_signed("example.test").is_ok()); }
}

async fn open(ctl: &mut stream::Control, peer: PeerId, port: u16) -> Result<tokio_util::compat::Compat<libp2p::Stream>> {
    let s = ctl.open_stream(peer, PROTO).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut s = s.compat();
    s.write_all(&port.to_be_bytes()).await?;
    Ok(s)
}

/// Worker side of health probes: report whether a deployed workload port accepts TCP connections.
pub async fn serve_health(mut control: stream::Control, trusted: Vec<PeerId>, allowed: Allowed) {
    let Ok(mut incoming) = control.accept(HEALTH) else { return };
    let sem = Arc::new(Semaphore::new(16));
    while let Some((peer, s)) = incoming.next().await {
        if !trusted.contains(&peer) { continue; }
        let Ok(permit) = sem.clone().try_acquire_owned() else { continue };
        let allowed = allowed.clone();
        tokio::spawn(async move {
            let _p = permit;
            let _ = async {
                let mut s = s.compat();
                let mut buf = [0u8; 2];
                tokio::time::timeout(Duration::from_secs(5), s.read_exact(&mut buf)).await??;
                let port = u16::from_be_bytes(buf);
                let ok = allowed.lock().unwrap().contains(&port)
                    && matches!(tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(("127.0.0.1", port))).await, Ok(Ok(_)));
                s.write_all(&[ok as u8]).await?;
                s.flush().await?;
                anyhow::Ok(())
            }.await;
        });
    }
}

/// Deployer side: true only if the worker answers and reports the workload healthy.
pub async fn probe(mut ctl: stream::Control, peer: PeerId, port: u16) -> bool {
    tokio::time::timeout(Duration::from_secs(5), async {
        let s = ctl.open_stream(peer, HEALTH).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut s = s.compat();
        s.write_all(&port.to_be_bytes()).await?;
        let mut b = [0u8; 1];
        s.read_exact(&mut b).await?;
        anyhow::Ok(b[0] == 1)
    })
    .await
    .ok()
    .and_then(|r| r.ok())
    .unwrap_or(false)
}
