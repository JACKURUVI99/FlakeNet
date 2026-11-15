// Mirrors the agent's /api/status JSON (peer-agent/src/status.rs).
export interface PeerInfo {
  peer_id: string
  connected: boolean
  trusted: boolean
  addr: string | null
  rtt_ms: number | null
  cpu_cores: number | null
  mem_total_kb: number | null
  mem_available_kb: number | null
  record_expires_in_secs: number | null
}
export interface WorkloadInfo { name: string; image: string; host_port: number; started_unix: number }
export interface DeploymentInfo {
  image: string
  state: 'scheduling' | 'deploying' | 'running' | 'failover' | 'failed'
  worker: string | null
  host_port: number
  container_port: number
  failed_probes: number
  gateway: string | null
  health: boolean[]
}
export type Level = 'info' | 'success' | 'warn' | 'error'
export interface EventInfo { ts_unix: number; level: Level; msg: string }
export interface Status {
  local_id: string
  version: string
  uptime_secs: number
  peers: PeerInfo[]
  workloads: WorkloadInfo[]
  deployment: DeploymentInfo | null
  events: EventInfo[]
}
