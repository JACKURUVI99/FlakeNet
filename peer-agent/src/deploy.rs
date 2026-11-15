//! Signed deployment requests and a Podman-backed OCI runner.
use std::{process::Command, time::{Duration, SystemTime, UNIX_EPOCH}};

use anyhow::{bail, Context, Result};
use libp2p::{identity, PeerId};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeployBody {
    pub image: String,
    pub host_port: u16,
    pub container_port: u16,
    pub cpus: f32,
    pub memory_mb: u32,
    pub nonce: u64,
    pub expires_unix: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeployRequest {
    pub body: String,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeployResponse {
    pub ok: bool,
    pub message: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub const REQ_CPUS: f32 = 1.0;
pub const REQ_MEM_MB: u32 = 256;

pub fn sign_request(kp: &identity::Keypair, image: &str, host_port: u16, container_port: u16) -> Result<DeployRequest> {
    let body = DeployBody {
        image: image.into(), host_port, container_port, cpus: REQ_CPUS, memory_mb: REQ_MEM_MB,
        nonce: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64,
        expires_unix: now() + 60,
    };
    let body = serde_json::to_string(&body)?;
    Ok(DeployRequest {
        signature: kp.sign(body.as_bytes())?,
        public_key: kp.public().encode_protobuf(),
        body,
    })
}

/// Verify signature, expiry and that the signer is trusted; return body.
pub fn authorize(req: &DeployRequest, from: &PeerId, trusted: &[PeerId]) -> Result<DeployBody> {
    let pk = identity::PublicKey::try_decode_protobuf(&req.public_key).context("bad key")?;
    let signer = pk.to_peer_id();
    if signer != *from { bail!("signer is not the connected peer"); }
    if !trusted.contains(&signer) { bail!("signer {signer} is not trusted"); }
    if !pk.verify(req.body.as_bytes(), &req.signature) { bail!("invalid signature"); }
    let b: DeployBody = serde_json::from_str(&req.body).context("bad body")?;
    if b.expires_unix < now() { bail!("request expired"); }
    if b.expires_unix > now() + 300 { bail!("request expiry too far in the future"); }
    validate(&b)?;
    Ok(b)
}

fn validate(b: &DeployBody) -> Result<()> {
    let ok_img = !b.image.is_empty()
        && b.image.len() < 256
        && !b.image.starts_with('-')
        && b.image.chars().all(|c| c.is_ascii_alphanumeric() || "/:._-@".contains(c));
    if !ok_img { bail!("invalid image reference"); }
    if b.host_port < 1024 { bail!("host port must be >= 1024"); }
    if b.container_port == 0 { bail!("container port must be set"); }
    if !(0.1..=8.0).contains(&b.cpus) || !(16..=8192).contains(&b.memory_mb) { bail!("resources out of range"); }
    Ok(())
}

/// Local worker policy.
#[derive(Clone)]
pub struct Policy {
    pub require_digest: bool,
    pub allowed_registries: Vec<String>,
    /// If set, images must verify with `cosign verify --key <path>` (and be digest-pinned).
    pub cosign_key: Option<String>,
    /// Requests signed before this agent started (nanos since epoch) are rejected as replays.
    pub min_nonce: u64,
    pub bind_addr: String,
}

/// Registry host of an image reference; bare names (`nginx`, `user/app`) are Docker Hub.
pub fn registry_of(image: &str) -> String {
    match image.split_once('/') {
        Some((first, _)) if first.contains('.') || first.contains(':') || first == "localhost" => first.to_string(),
        _ => "docker.io".to_string(),
    }
}

pub fn check_policy(b: &DeployBody, pol: &Policy) -> Result<()> {
    if b.nonce < pol.min_nonce { bail!("request predates this agent's start (possible replay)"); }
    let reg = registry_of(&b.image);
    if !pol.allowed_registries.iter().any(|r| *r == reg) {
        bail!("registry {reg} is not allowed");
    }
    if (pol.require_digest || pol.cosign_key.is_some()) && !b.image.contains("@sha256:") {
        bail!("policy requires digest-pinned image (name@sha256:...)");
    }
    Ok(())
}

/// Fail-closed signature check: any error (including cosign missing) denies the deploy.
async fn verify_signature(image: &str, key: &str) -> Result<()> {
    let out = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("cosign").args(["verify", "--key", key, "--", image]).kill_on_drop(true).output(),
    )
    .await
    .context("cosign timed out")?
    .context("cannot run cosign (is it installed?)")?;
    if !out.status.success() {
        bail!("image signature verification failed: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").trim());
    }
    Ok(())
}

/// Starts the container without blocking the event loop; returns its name.
pub async fn run(b: DeployBody, pol: Policy) -> Result<String> {
    if let Some(key) = &pol.cosign_key { verify_signature(&b.image, key).await?; }
    let name = format!("flakenet-{}-{}", b.host_port, b.nonce);
    let mut cmd = tokio::process::Command::new("podman");
    cmd.args(["run", "-d", "--name", &name, "--cap-drop=ALL", "--security-opt=no-new-privileges", "--pids-limit=256"])
        // Minimal caps so stock images that start as root then drop to a service user (nginx, etc.) work.
        .args(["--cap-add=CHOWN", "--cap-add=SETUID", "--cap-add=SETGID", "--cap-add=NET_BIND_SERVICE"])
        .args(["--cpus", &b.cpus.to_string(), "--memory", &format!("{}m", b.memory_mb)])
        .args(["-p", &format!("{}:{}:{}", pol.bind_addr, b.host_port, b.container_port)])
        .args(["--label", "flakenet=1", "--", &b.image])
        .kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(180), cmd.output())
        .await
        .context("podman run timed out")?
        .context("spawning podman")?;
    if !out.status.success() {
        remove(&name);
        bail!("podman failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(name)
}

pub fn remove(name: &str) {
    let _ = Command::new("podman").args(["rm", "-f", "--", name]).output();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_signer_rejected() {
        let a = identity::Keypair::generate_ed25519();
        let r = sign_request(&a, "docker.io/library/nginx:alpine", 8080, 80).unwrap();
        assert!(authorize(&r, &a.public().to_peer_id(), &[]).is_err());
        assert!(authorize(&r, &a.public().to_peer_id(), &[a.public().to_peer_id()]).is_ok());
    }

    #[test]
    fn spoofed_sender_and_bad_input_rejected() {
        let a = identity::Keypair::generate_ed25519();
        let m = identity::Keypair::generate_ed25519();
        let t = [a.public().to_peer_id()];
        let r = sign_request(&a, "nginx", 8080, 80).unwrap();
        assert!(authorize(&r, &m.public().to_peer_id(), &t).is_err()); // replay by other peer
        for (img, port) in [("--privileged", 8080), ("nginx;rm -rf", 8080), ("nginx", 80)] {
            let r = sign_request(&a, img, port, 80).unwrap();
            assert!(authorize(&r, &a.public().to_peer_id(), &t).is_err(), "{img}:{port}");
        }
    }

    fn pol() -> Policy {
        Policy { require_digest: false, allowed_registries: vec!["docker.io".into(), "ghcr.io".into()],
                 cosign_key: None, min_nonce: 0, bind_addr: "127.0.0.1".into() }
    }
    fn body(image: &str, nonce: u64) -> DeployBody {
        DeployBody { image: image.into(), host_port: 8080, container_port: 80, cpus: 1.0, memory_mb: 256, nonce, expires_unix: now() + 60 }
    }

    #[test]
    fn registry_parsing() {
        assert_eq!(registry_of("nginx"), "docker.io");
        assert_eq!(registry_of("user/app:1"), "docker.io");
        assert_eq!(registry_of("docker.io/library/nginx:alpine"), "docker.io");
        assert_eq!(registry_of("ghcr.io/x/y@sha256:ab"), "ghcr.io");
        assert_eq!(registry_of("evil.example.com:5000/x"), "evil.example.com:5000");
        assert_eq!(registry_of("localhost/x"), "localhost");
    }

    #[test]
    fn registry_allowlist_enforced() {
        assert!(check_policy(&body("ghcr.io/a/b:1", 1), &pol()).is_ok());
        assert!(check_policy(&body("nginx", 1), &pol()).is_ok());
        assert!(check_policy(&body("evil.example.com/a:1", 1), &pol()).is_err());
        assert!(check_policy(&body("quay.io/a:1", 1), &pol()).is_err());
    }

    #[test]
    fn cosign_requires_digest_and_replay_floor() {
        let mut p = pol();
        p.cosign_key = Some("k.pub".into());
        assert!(check_policy(&body("nginx:alpine", 1), &p).is_err());
        assert!(check_policy(&body("nginx@sha256:abc", 1), &p).is_ok());
        let mut p = pol();
        p.min_nonce = 100;
        assert!(check_policy(&body("nginx", 99), &p).is_err());
        assert!(check_policy(&body("nginx", 100), &p).is_ok());
    }
}
