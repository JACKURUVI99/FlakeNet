import type { Status } from './types'

// Sample data for `?demo` — lets the UI be developed and screenshotted without a running agent.
export function demoStatus(): Status {
  const now = Math.floor(Date.now() / 1000)
  const peer = (id: string, cpu: number, gb: number, free: number, rtt: number, trusted: boolean, connected = true) => ({
    peer_id: id, connected, trusted, addr: `/ip4/10.0.${cpu}.${gb}/udp/4001/quic-v1`, rtt_ms: connected ? rtt : null,
    cpu_cores: cpu, mem_total_kb: gb * 1048576, mem_available_kb: Math.round(gb * 1048576 * free), record_expires_in_secs: 41,
  })
  return {
    local_id: '12D3KooWEdTfutt7vitcWYdw4b5TXXT67NhVjS938YyyM6DXUUd7', version: '0.1.0', uptime_secs: 5423,
    peers: [
      peer('12D3KooWJsuj7oQkSUsc5g7mBjAU4ocn9tppFTTsgShmbgcGwffJ', 12, 32, 0.62, 1.2, true),
      peer('12D3KooWR8pMCuEAtExWbG2ruTvUc7GHhJzzfnvMjKCuq78py9ZK', 8, 16, 0.35, 3.8, true),
      peer('12D3KooWKknuyzWHS8seF3rJK9wGZ1WQUboRjHBrZrehTuuGBbLW', 4, 8, 0.8, 21.4, false),
      peer('12D3KooWAwzjzH4bKuJwo4K6giHnm6iWcWbHeSEW5dCXpSdbSRmb', 16, 64, 0.5, 8.9, true),
      peer('12D3KooWN96w5249FLc5kD9c7DPQ8B4Ee45EG4QW6YidbVvmJSLV', 2, 4, 0.2, 55.0, false),
      peer('12D3KooWBCDC9SyC2o7vgRQgE1uC1hJL4tC1idRVDWtq5EZEBtn5', 6, 12, 0.7, 0, false, false),
    ],
    workloads: [],
    deployment: {
      image: 'docker.io/library/nginx:alpine', state: 'running', worker: '12D3KooWJsuj7oQkSUsc5g7mBjAU4ocn9tppFTTsgShmbgcGwffJ',
      host_port: 8080, container_port: 80, failed_probes: 0, gateway: 'https://127.0.0.1:9000',
      health: [true, true, true, true, true, false, false, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true],
    },
    events: [
      { ts_unix: now - 6, level: 'success', msg: 'gateway now routes to 12D3KooW…wffJ' },
      { ts_unix: now - 9, level: 'error', msg: 'worker 12D3KooW…ZK is unhealthy: failing over' },
      { ts_unix: now - 41, level: 'warn', msg: 'health probe failed for 12D3KooW…ZK (2)' },
      { ts_unix: now - 300, level: 'success', msg: 'deployed docker.io/library/nginx:alpine on :8080' },
      { ts_unix: now - 304, level: 'info', msg: 'scheduled docker.io/library/nginx:alpine on 12D3KooW…ZK' },
      { ts_unix: now - 308, level: 'error', msg: 'denied deploy from 12D3KooW…VrNc: signer is not trusted' },
      { ts_unix: now - 600, level: 'info', msg: 'peer connected 12D3KooW…wffJ' },
    ],
  }
}
