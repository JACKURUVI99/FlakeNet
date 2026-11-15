export const shortId = (id: string) => (id.length > 14 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id)

export function fmtKb(kb: number | null | undefined): string {
  if (kb == null) return '—'
  const gb = kb / 1048576
  return gb >= 1 ? `${gb.toFixed(gb >= 10 ? 0 : 1)} GB` : `${Math.round(kb / 1024)} MB`
}

export function fmtMs(ms: number | null | undefined): string {
  if (ms == null) return '—'
  return ms < 10 ? `${ms.toFixed(1)} ms` : `${Math.round(ms)} ms`
}

export function timeAgo(unix: number, now = Date.now() / 1000): string {
  const s = Math.max(0, Math.round(now - unix))
  if (s < 5) return 'just now'
  if (s < 60) return `${s}s ago`
  if (s < 3600) return `${Math.floor(s / 60)}m ago`
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`
  return `${Math.floor(s / 86400)}d ago`
}

export function fmtUptime(secs: number): string {
  const h = Math.floor(secs / 3600), m = Math.floor((secs % 3600) / 60)
  return h > 0 ? `${h}h ${m}m` : `${m}m ${secs % 60}s`
}

/** Deterministic 32-bit hash (FNV-1a) used to derive each peer's identicon. */
export function hash32(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619) }
  return h >>> 0
}
