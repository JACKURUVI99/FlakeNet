import { fmtKb, fmtMs, fmtUptime, shortId, timeAgo } from './format'
import type { DeploymentInfo, EventInfo, PeerInfo, Status } from './types'
import { EventIcon, Identicon, peerColors, Sparkline, StatePill, VerifiedIcon } from './ui'

/* ---------- Peer card ---------- */
export function PeerCard({ p }: { p: PeerInfo }) {
  const [a, b] = peerColors(p.peer_id)
  const free = p.mem_total_kb && p.mem_available_kb != null ? p.mem_available_kb / p.mem_total_kb : null
  return (
    <article className={`card${p.connected ? '' : ' offline'}`}>
      <div className="media" style={{ background: `linear-gradient(135deg, ${a}, ${b})` }}>
        <span className={`badge${p.trusted ? ' trusted' : ''}`}>
          <i className={`dot ${p.connected ? 'ok' : 'bad'}`} />{p.connected ? (p.trusted ? 'Trusted' : 'Connected') : 'Offline'}
        </span>
        <Identicon id={p.peer_id} />
      </div>
      <div className="card-body">
        <div className="card-title" title={p.peer_id}>{shortId(p.peer_id)}{p.trusted && <VerifiedIcon />}</div>
        <div className="stats">
          <div className="stat"><div className="k">CPU</div><div className="v num">{p.cpu_cores != null ? `${p.cpu_cores} cores` : '—'}</div></div>
          <div className="stat"><div className="k">Free memory</div><div className="v num">{fmtKb(p.mem_available_kb)}</div></div>
        </div>
        {free != null && <div className={`bar${free < 0.25 ? ' low' : ''}`} title={`${Math.round(free * 100)}% free`}><i style={{ width: `${Math.round(free * 100)}%` }} /></div>}
        <div className="card-foot"><span className="num">Ping {fmtMs(p.rtt_ms)}</span><span className="num">of {fmtKb(p.mem_total_kb)}</span></div>
      </div>
    </article>
  )
}

/* ---------- Events ---------- */
export function Feed({ events, limit }: { events: EventInfo[]; limit?: number }) {
  const list = limit ? events.slice(0, limit) : events
  if (list.length === 0) return <div className="feed"><div className="empty">No activity yet.</div></div>
  return (
    <div className="feed">
      {list.map((e, i) => (
        <div className="row" key={`${e.ts_unix}-${i}`}>
          <EventIcon level={e.level} />
          <div className="msg">{e.msg}</div>
          <time dateTime={new Date(e.ts_unix * 1000).toISOString()}>{timeAgo(e.ts_unix)}</time>
        </div>
      ))}
    </div>
  )
}

/* ---------- Hero ---------- */
export function Hero({ d, s }: { d: DeploymentInfo | null; s: Status }) {
  if (!d) {
    return (
      <section className="hero hero-empty">
        <div>
          <h1>No deployment yet</h1>
          <p className="muted">This agent isn’t scheduling a workload. Start one from the CLI:</p>
          <code className="cmd">flakenet --bootstrap &lt;worker&gt; --deploy-image nginx:alpine --gateway-listen 127.0.0.1:9000</code>
        </div>
      </section>
    )
  }
  const ok = d.health.filter(Boolean).length
  return (
    <section className="hero">
      <div>
        <StatePill state={d.state} />
        <h1>{d.image.split('/').pop()}</h1>
        <div className="muted mono" style={{ fontSize: 12 }}>{d.image}</div>
        <div className="meta">
          <span className="chip mono">:{d.host_port} → {d.container_port}</span>
          {d.worker && <span className="chip mono" title={d.worker}>on {shortId(d.worker)}</span>}
          {d.gateway && <a className="chip mono" href={d.gateway} target="_blank" rel="noreferrer noopener">{d.gateway}</a>}
        </div>
      </div>
      <div className="hero-stats">
        <div className="hero-stat"><div className="k">Health checks</div><div className="v num">{d.health.length ? `${Math.round((ok / d.health.length) * 100)}%` : '—'}</div></div>
        <div className="hero-stat"><div className="k">Failed probes</div><div className="v num">{d.failed_probes}</div></div>
        <div className="hero-stat" style={{ gridColumn: '1 / -1' }}><div className="k">Recent health</div><div style={{ marginTop: 8 }}><Sparkline values={d.health} /></div></div>
        <div className="hero-stat" style={{ gridColumn: '1 / -1' }}><div className="k">Agent uptime</div><div className="v num">{fmtUptime(s.uptime_secs)}</div></div>
      </div>
    </section>
  )
}

/* ---------- Deployments table ---------- */
export function Deployments({ s, q }: { s: Status; q: string }) {
  const rows: { key: string; image: string; port: string; where: string; state: string; health: boolean[] }[] = []
  const d = s.deployment
  if (d) rows.push({ key: 'dep', image: d.image, port: `:${d.host_port} → ${d.container_port}`, where: d.worker ? shortId(d.worker) : 'unplaced', state: d.state, health: d.health })
  for (const w of s.workloads) rows.push({ key: w.name, image: w.image, port: `:${w.host_port}`, where: 'this node', state: 'running', health: [] })
  const f = rows.filter((r) => `${r.image} ${r.where}`.toLowerCase().includes(q.toLowerCase()))
  if (f.length === 0) return <div className="feed"><div className="empty">No workloads{q ? ' match your search' : ' yet'}.</div></div>
  return (
    <div className="table-wrap">
      <table>
        <thead><tr><th>#</th><th>Workload</th><th>Runs on</th><th>State</th><th>Health</th><th className="r">Port</th></tr></thead>
        <tbody>
          {f.map((r, i) => (
            <tr key={r.key}>
              <td className="muted num">{i + 1}</td>
              <td><div className="cell-main"><span className="thumb">{(r.image.split('/').pop() ?? '?')[0].toUpperCase()}</span><div><div className="name">{r.image.split('/').pop()}</div><div className="sub mono">{r.image}</div></div></div></td>
              <td className="mono">{r.where}</td>
              <td><StatePill state={r.state} /></td>
              <td><Sparkline values={r.health} /></td>
              <td className="r mono num">{r.port}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
