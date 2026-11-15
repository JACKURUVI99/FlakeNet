<p align="center"><img src="assets/logo.png" alt="FlakeNet logo" width="160"></p>

# FlakeNet

Decentralized cloud platform: deploy OCI apps and VMs across independently operated peers.
Build order: P2P networking → container execution → ingress → isolation → ZK eligibility → scheduler → storage → serverless → marketplace.

## Status
`peer-agent/` (Rust, libp2p) implements:
- Ed25519 identity (0600 key file), QUIC transport, Kademlia discovery
- Signed, expiring resource records (CPU/RAM), verified by receivers
- Signed deploy requests, trust allowlist, Podman runner (rootless, caps dropped, loopback-bound)
- Scheduler: eligibility filter + best-fit ranking with fallback/rescheduling
- Authenticated TCP tunnel + local HTTP/HTTPS gateway (TLS, rate limit, idle timeout) to the scheduled worker
- Health probes with automatic failover: unhealthy worker -> reschedule elsewhere, gateway repointed (~5s)

## Test
    cd peer-agent && cargo test          # unit tests
    tests/integration/e2e.sh 3           # end-to-end, 3 runs (needs podman)

## Worker policy
`--trust <peer-id>` (required to accept deploys), `--allow-registry` (default docker.io, ghcr.io, quay.io),
`--require-digest`, `--cosign-key <pub>` (fail-closed signature check; implies digest-pinning),
`--max-workloads`, `--workload-bind`. Gateway: `--gateway-listen`, `--tls-cert/--tls-key` or `--tls-self-signed`, `--rate-limit`.

## Known gaps
- Failover does not fence the old worker (partition => two copies); fine for stateless apps, unsafe for databases.
- Workers answer resource-record requests from any peer (discloses CPU/RAM figures).
- Resource claims are self-reported (no attestation/challenges yet).
- `--cosign-key` is implemented but only its fail-closed path is tested (cosign is not installed on the dev box).
- Replay floor assumes roughly synchronized clocks between deployer and worker.
- Gateway: no ACME, no per-request limiting, no WAF; rate limit is per connection and per source IP.
- Podman isolation is not microVM-grade; suitable for trusted private peers only.
- Only tested on one machine (loopback QUIC), not across hosts/NAT.

## Credits
Logo mark: "S & Snow" logomark by @nextmahamud, found on Pinterest (https://in.pinterest.com/pin/135389532543855420/). Not created by this project; confirm usage rights before any public release.
