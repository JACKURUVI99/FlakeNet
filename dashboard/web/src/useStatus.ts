import { useEffect, useState } from 'react'
import { demoStatus } from './demo'
import type { Status } from './types'

export const isDemo = new URLSearchParams(location.search).has('demo')

/** Polls the agent's read-only status API. */
export function useStatus(intervalMs = 2000) {
  const [status, setStatus] = useState<Status | null>(isDemo ? demoStatus() : null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (isDemo) return
    let alive = true
    const ctl = new AbortController()
    const tick = async () => {
      try {
        const r = await fetch('/api/status', { signal: ctl.signal, cache: 'no-store' })
        if (!r.ok) throw new Error(`HTTP ${r.status}`)
        const j = (await r.json()) as Status
        if (alive) { setStatus(j); setError(null) }
      } catch (e) {
        if (alive && !ctl.signal.aborted) setError(e instanceof Error ? e.message : 'request failed')
      }
    }
    void tick()
    const t = setInterval(tick, intervalMs)
    return () => { alive = false; ctl.abort(); clearInterval(t) }
  }, [intervalMs])

  return { status, error, online: status !== null && error === null }
}
