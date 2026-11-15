//! Two-stage placement: hard eligibility filter, then best-fit ranking.
//! Pure functions so they are easy to test and later swap for smarter policies.
use std::time::{SystemTime, UNIX_EPOCH};

use libp2p::PeerId;

use crate::record::Body;

pub struct Requirements {
    pub cpus: f32,
    pub memory_mb: u32,
}

/// Keep 20% headroom over the requested memory so we don't place on a nearly-full node.
const MEM_HEADROOM: f64 = 1.2;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Stage 1: mandatory constraints.
pub fn eligible(b: &Body, req: &Requirements) -> bool {
    let need_kb = (req.memory_mb as f64 * 1024.0 * MEM_HEADROOM) as u64;
    b.expires_unix >= now()
        && b.cpu_cores as f32 >= req.cpus
        && b.mem_available_kb >= need_kb
}

/// Stage 2: best-fit (least leftover memory that still fits), ties broken by peer id
/// for determinism. Best-fit also avoids rewarding peers that over-claim capacity.
/// Returns candidates best-first so callers can fall back.
pub fn rank(cands: &[(PeerId, Body)], req: &Requirements) -> Vec<PeerId> {
    let mut v: Vec<&(PeerId, Body)> = cands.iter().filter(|(_, b)| eligible(b, req)).collect();
    v.sort_by(|a, b| {
        a.1.mem_available_kb
            .cmp(&b.1.mem_available_kb)
            .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
    });
    v.into_iter().map(|(p, _)| *p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    fn cand(cpu: u32, mem_kb: u64, ttl: i64) -> (PeerId, Body) {
        let p = Keypair::generate_ed25519().public().to_peer_id();
        let b = Body {
            peer_id: p.to_string(), cpu_cores: cpu, mem_total_kb: mem_kb * 2,
            mem_available_kb: mem_kb, seq: 1, expires_unix: (now() as i64 + ttl) as u64,
        };
        (p, b)
    }
    const REQ: Requirements = Requirements { cpus: 1.0, memory_mb: 256 };

    #[test]
    fn filters_ineligible() {
        let small = cand(4, 100 * 1024, 30); // too little memory
        let nocpu = cand(0, 8 * 1024 * 1024, 30);
        let stale = cand(4, 8 * 1024 * 1024, -5);
        let ok = cand(4, 8 * 1024 * 1024, 30);
        let r = rank(&[small, nocpu, stale, ok.clone()], &REQ);
        assert_eq!(r, vec![ok.0]);
    }

    #[test]
    fn best_fit_orders_by_least_leftover() {
        let big = cand(8, 16 * 1024 * 1024, 30);
        let mid = cand(8, 2 * 1024 * 1024, 30);
        let r = rank(&[big.clone(), mid.clone()], &REQ);
        assert_eq!(r, vec![mid.0, big.0]);
    }

    #[test]
    fn none_when_nothing_fits() {
        assert!(rank(&[cand(1, 1024, 30)], &REQ).is_empty());
    }
}
