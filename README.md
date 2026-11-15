<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo.svg">
    <img src="assets/logo-light.svg" alt="FlakeNet logo" width="160">
  </picture>
</p>

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

## Quick start
    cd peer-agent && cargo build              # produces target/debug/flakenet
    flakenet init                             # writes flakenet.toml (all settings, commented)
    flakenet id                               # prints this machine's peer ID (creates the key)
    flakenet --config flakenet.toml           # run the agent

Worker: set `trust = ["<deployer peer id>"]` in its config. Deployer: set `bootstrap = ["<worker multiaddr>/p2p/<id>"]`,
`deploy-image`, `deploy-ports` and `gateway-listen`. Command-line flags override the config file; unknown config keys are rejected.
Config can also come from `$FLAKENET_CONFIG`.

## Dashboard
A React + TypeScript web UI (dark marketplace-style theme; analysis in `docs/design/opensea-analysis.md`).

    cd dashboard/web && npm install && npm run build
    cd ../.. && ./peer-agent/target/debug/flakenet --api-listen 127.0.0.1:7777 --dashboard-dir dashboard/web/dist
    # open https://127.0.0.1:7777      (add ?demo to preview with sample data)

**HTTPS by default.** A local certificate for `localhost`, `127.0.0.1` and `::1` is generated on first run and stored next to the
key file (`<key-file>.api.crt` / `.api.key`, key mode 0600); it is reused on restart. Browsers warn about it until you trust that
`.crt` (import it into your browser/OS, or `curl --cacert <file>`). Bring your own with `--api-tls-cert/--api-tls-key`.
Plain HTTP is an explicit opt-out: `--api-http`.

Dev server with hot reload: `npm run dev` (proxies `/api` to the agent; start the agent with `--api-http` for that).
The API is read-only and unauthenticated, so it only binds to loopback (refuses otherwise unless `--api-allow-remote`),
validates the `Host` header (anti DNS-rebinding) and sets strict security headers.

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
