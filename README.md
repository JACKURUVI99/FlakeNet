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

## Worker policy
`--trust <peer-id>` (required to accept deploys), `--allow-registry` (default docker.io, ghcr.io, quay.io),
`--require-digest`, `--cosign-key <pub>` (fail-closed signature check; implies digest-pinning),
`--max-workloads`, `--workload-bind`. Gateway: `--gateway-listen`, `--tls-cert/--tls-key` or `--tls-self-signed`, `--rate-limit`.

## Known gaps
- Resource claims are self-reported (no attestation/challenges yet).
- `--cosign-key` is implemented but only its fail-closed path is tested (cosign is not installed on the dev box).
- Replay floor assumes roughly synchronized clocks between deployer and worker.
- Gateway: no ACME, no per-request limiting, no WAF; rate limit is per connection and per source IP.
- Podman isolation is not microVM-grade; suitable for trusted private peers only.
- Only tested on one machine (loopback QUIC), not across hosts/NAT.
