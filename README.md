# FlakeNet

Decentralized cloud platform: deploy OCI apps and VMs across independently operated peers.
Build order: P2P networking → container execution → ingress → isolation → ZK eligibility → scheduler → storage → serverless → marketplace.

## Status
`peer-agent/` (Rust, libp2p) implements:
- Ed25519 identity (0600 key file), QUIC transport, Kademlia discovery
- Signed, expiring resource records (CPU/RAM), verified by receivers
- Signed deploy requests, trust allowlist, Podman runner (rootless, caps dropped, loopback-bound)
- Scheduler: eligibility filter + best-fit ranking with fallback/rescheduling
- Authenticated TCP tunnel + local HTTP gateway to the scheduled worker

## Test
    cd peer-agent && cargo test          # unit tests
    tests/integration/e2e.sh 3           # end-to-end, 3 runs (needs podman)

## Known gaps
Resource claims are self-reported; no image signature/registry policy yet; no TLS/rate limiting on the gateway;
tunnels have no idle timeout; Podman isolation is not microVM-grade.
