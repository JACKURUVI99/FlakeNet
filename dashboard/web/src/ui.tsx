import type { ReactNode } from 'react'
import { hash32 } from './format'
import type { Level } from './types'

/* ---------- icons (inline, stroke-based) ---------- */
const I = ({ children, size = 18 }: { children: ReactNode; size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{children}</svg>
)
export const SearchIcon = () => <I><circle cx="11" cy="11" r="7" /><path d="m21 21-4.3-4.3" /></I>
export const SunIcon = () => <I><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></I>
export const MoonIcon = () => <I><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z" /></I>
export const CopyIcon = () => <I size={16}><rect x="9" y="9" width="12" height="12" rx="2" /><path d="M5 15V5a2 2 0 0 1 2-2h10" /></I>
export const VerifiedIcon = () => (
  <svg className="verified" width="16" height="16" viewBox="0 0 24 24" aria-label="Trusted peer"><path fill="currentColor" d="M12 1.8 14.6 4l3.4-.2.9 3.3 2.9 1.8-1 3.2 1 3.2-2.9 1.8-.9 3.3-3.4-.2L12 22.2 9.4 20l-3.4.2-.9-3.3-2.9-1.8 1-3.2-1-3.2 2.9-1.8.9-3.3L9.4 4z" /><path d="m8.5 12.2 2.4 2.4 4.7-5" fill="none" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" /></svg>
)
const evIcons: Record<Level, ReactNode> = {
  info: <I size={16}><circle cx="12" cy="12" r="9" /><path d="M12 8h.01M11 12h1v4h1" /></I>,
  success: <I size={16}><path d="m5 12.5 4.5 4.5L19 7.5" /></I>,
  warn: <I size={16}><path d="M12 3 2 20h20L12 3Z" /><path d="M12 10v4M12 17h.01" /></I>,
  error: <I size={16}><circle cx="12" cy="12" r="9" /><path d="m9 9 6 6M15 9l-6 6" /></I>,
}
export const EventIcon = ({ level }: { level: Level }) => <span className={`ev-icon ${level}`}>{evIcons[level]}</span>

/* ---------- identicon: a unique, deterministic avatar per peer ---------- */
export function peerColors(id: string): [string, string] {
  const h = hash32(id)
  const hue = h % 360
  return [`hsl(${hue} 70% 55%)`, `hsl(${(hue + 55) % 360} 75% 45%)`]
}

export function Identicon({ id }: { id: string }) {
  const h = hash32(id), h2 = hash32(id + '#')
  const cells: [number, number][] = []
  for (let y = 0; y < 5; y++) for (let x = 0; x < 3; x++) {
    if (((x < 2 ? h : h2) >> (y * 3 + x)) & 1) { cells.push([x, y]); if (x < 2) cells.push([4 - x, y]) }
  }
  return (
    <svg className="ident" viewBox="0 0 5 5" aria-hidden="true">
      <path fill="#fff" opacity="0.92" d={cells.map(([x, y]) => `M${x} ${y}h1v1h-1z`).join("")} />
    </svg>
  )
}

export function Sparkline({ values }: { values: boolean[] }) {
  if (values.length === 0) return <span className="muted">—</span>
  return (
    <span className="spark" role="img" aria-label={`${values.filter(Boolean).length} of ${values.length} health checks passed`}>
      {values.slice(-30).map((ok, i) => <i key={i} className={ok ? '' : 'bad'} />)}
    </span>
  )
}

export const StatePill = ({ state }: { state: string }) => <span className={`state ${state}`}>{state}</span>
