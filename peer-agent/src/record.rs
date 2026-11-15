//! Signed resource advertisements. A peer signs a body describing its resources;
//! receivers verify signature, peer-id binding and expiry before trusting it.
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use libp2p::{identity, PeerId};
use serde::{Deserialize, Serialize};

pub const TTL_SECS: u64 = 60;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Body {
    pub peer_id: String,
    pub cpu_cores: u32,
    pub mem_total_kb: u64,
    pub mem_available_kb: u64,
    pub seq: u64,
    pub expires_unix: u64,
}

/// `body` is the exact JSON string that was signed (avoids canonicalisation issues).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedRecord {
    pub body: String,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn meminfo(field: &str) -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(field))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

pub fn create(kp: &identity::Keypair, seq: u64) -> Result<SignedRecord> {
    let body = Body {
        peer_id: kp.public().to_peer_id().to_string(),
        cpu_cores: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1),
        mem_total_kb: meminfo("MemTotal:"),
        mem_available_kb: meminfo("MemAvailable:"),
        seq,
        expires_unix: now() + TTL_SECS,
    };
    sign(kp, &body)
}

pub fn sign(kp: &identity::Keypair, body: &Body) -> Result<SignedRecord> {
    let body = serde_json::to_string(body)?;
    Ok(SignedRecord {
        signature: kp.sign(body.as_bytes())?,
        public_key: kp.public().encode_protobuf(),
        body,
    })
}

/// Verify against the peer we expect the record to come from.
pub fn verify(rec: &SignedRecord, expected: &PeerId) -> Result<Body> {
    let pk = identity::PublicKey::try_decode_protobuf(&rec.public_key).context("bad public key")?;
    if pk.to_peer_id() != *expected {
        bail!("public key does not match peer id");
    }
    if !pk.verify(rec.body.as_bytes(), &rec.signature) {
        bail!("invalid signature");
    }
    let body: Body = serde_json::from_str(&rec.body).context("bad body")?;
    if body.peer_id != expected.to_string() {
        bail!("body peer_id mismatch");
    }
    if body.expires_unix < now() {
        bail!("record expired");
    }
    if body.expires_unix > now() + 2 * TTL_SECS {
        bail!("record expiry too far in the future");
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_record_verifies() {
        let kp = identity::Keypair::generate_ed25519();
        let rec = create(&kp, 1).unwrap();
        let b = verify(&rec, &kp.public().to_peer_id()).unwrap();
        assert!(b.cpu_cores >= 1 && b.mem_total_kb > 0);
    }

    #[test]
    fn tampered_body_rejected() {
        let kp = identity::Keypair::generate_ed25519();
        let mut rec = create(&kp, 1).unwrap();
        rec.body = rec.body.replace("\"cpu_cores\":", "\"cpu_cores\":9");
        assert!(verify(&rec, &kp.public().to_peer_id()).is_err());
    }

    #[test]
    fn wrong_peer_rejected() {
        let kp = identity::Keypair::generate_ed25519();
        let other = identity::Keypair::generate_ed25519();
        let rec = create(&kp, 1).unwrap();
        assert!(verify(&rec, &other.public().to_peer_id()).is_err());
    }

    #[test]
    fn far_future_expiry_rejected() {
        let kp = identity::Keypair::generate_ed25519();
        let body = Body {
            peer_id: kp.public().to_peer_id().to_string(),
            cpu_cores: 1, mem_total_kb: 1, mem_available_kb: 1, seq: 1,
            expires_unix: now() + 10_000_000,
        };
        assert!(verify(&sign(&kp, &body).unwrap(), &kp.public().to_peer_id()).is_err());
    }

    #[test]
    fn forged_identity_rejected() {
        // Attacker signs a body claiming the victim's peer_id with their own key.
        let victim = identity::Keypair::generate_ed25519();
        let attacker = identity::Keypair::generate_ed25519();
        let mut body = Body {
            peer_id: victim.public().to_peer_id().to_string(),
            cpu_cores: 999, mem_total_kb: 1, mem_available_kb: 1, seq: 1,
            expires_unix: now() + 60,
        };
        let rec = sign(&attacker, &body).unwrap();
        assert!(verify(&rec, &victim.public().to_peer_id()).is_err());
        body.expires_unix = 1;
        let rec = sign(&victim, &body).unwrap();
        assert!(verify(&rec, &victim.public().to_peer_id()).is_err()); // expired
    }
}
