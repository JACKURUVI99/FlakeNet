import { useEffect, useMemo, useState } from 'react'
import { fmtKb, fmtUptime, shortId } from './format'
import { Deployments, Feed, Hero, PeerCard } from './pages'
import { CopyIcon, Identicon, MoonIcon, peerColors, SearchIcon, SunIcon } from './ui'
import { isDemo, useStatus } from './useStatus'
import './theme.css'

type Tab = 'overview' | 'peers' | 'deployments' | 'activity'
type Filter = 'all' | 'connected' | 'trusted'
type Sort = 'memory' | 'latency' | 'cpu'

function useTheme() {
  // Dark by default (marketplace look); the user's choice is remembered.
  const [theme, setTheme] = useState<'dark' | 'light'>(() => {
    try { return localStorage.getItem('flakenet-theme') === 'light' ? 'light' : 'dark' } catch { return 'dark' }
  })
  useEffect(() => {
    document.documentElement.dataset.theme = theme
    try { localStorage.setItem('flakenet-theme', theme) } catch { /* storage unavailable */ }
  }, [theme])
  const isLight = theme === 'light'
  return { isLight, toggle: () => setTheme(isLight ? 'dark' : 'light') }
}

export default function App() {
  const { status, error, online } = useStatus()
  const { isLight, toggle } = useTheme()
  const [tab, setTab] = useState<Tab>(() => { const h = location.hash.slice(1) as Tab; return ['overview', 'peers', 'deployments', 'activity'].includes(h) ? h : 'overview' })
  const [q, setQ] = useState('')
  const [filter, setFilter] = useState<Filter>('all')
  const [sort, setSort] = useState<Sort>('memory')
  const [copied, setCopied] = useState(false)

  const peers = useMemo(() => {
    const list = (status?.peers ?? []).filter((p) =>
      p.peer_id.toLowerCase().includes(q.toLowerCase()) && (filter === 'all' || (filter === 'connected' ? p.connected : p.trusted)))
    const key = (p: (typeof list)[number]) => sort === 'memory' ? -(p.mem_available_kb ?? -1) : sort === 'cpu' ? -(p.cpu_cores ?? -1) : (p.rtt_ms ?? 1e9)
    return [...list].sort((a, b) => key(a) - key(b))
  }, [status, q, filter, sort])

  const copyId = async () => {
    if (!status) return
    try { await navigator.clipboard.writeText(status.local_id); setCopied(true); setTimeout(() => setCopied(false), 1500) } catch { /* clipboard blocked */ }
  }
  const onSearch = (v: string) => { setQ(v); if (v && tab === 'overview') setTab('peers') }

  const up = status?.peers.filter((p) => p.connected) ?? []
  const cores = up.reduce((n, p) => n + (p.cpu_cores ?? 0), 0)
  const freeKb = up.reduce((n, p) => n + (p.mem_available_kb ?? 0), 0)
  const events = (status?.events ?? []).filter((e) => e.msg.toLowerCase().includes(q.toLowerCase()))
  const tabs: [Tab, string, number?][] = [
    ['overview', 'Overview'], ['peers', 'Peers', status?.peers.length], ['deployments', 'Deployments', (status?.workloads.length ?? 0) + (status?.deployment ? 1 : 0)], ['activity', 'Activity'],
  ]

  return (
    <>
      <header className="nav">
        <a className="brand" href="/" aria-label="FlakeNet home">
          <img className="logo-dark" src="/logo.svg" alt="" /><img className="logo-light" src="/logo-light.svg" alt="" /><span>FlakeNet</span>
        </a>
        <label className="search">
          <SearchIcon /><span className="sr-only">Search</span>
          <input value={q} onChange={(e) => onSearch(e.target.value)} placeholder="Search peers, workloads, activity" />
        </label>
        <div className="nav-right">
          <span className="pill-btn" title={error ?? 'Agent reachable'}><i className={`dot ${online || isDemo ? 'ok' : 'bad'}`} /><span className="lbl">{isDemo ? 'Demo data' : online ? 'Agent online' : 'Agent offline'}</span></span>
          <button className="pill-btn primary" onClick={copyId} disabled={!status}><CopyIcon /><span className="lbl">{copied ? 'Copied' : 'Copy peer ID'}</span></button>
          <button className="pill-btn icon-btn" onClick={toggle} aria-label="Toggle theme">{isLight ? <MoonIcon /> : <SunIcon />}</button>
        </div>
      </header>

      <nav className="tabs" role="tablist" aria-label="Sections">
        {tabs.map(([id, label, n]) => (
          <button key={id} role="tab" className="tab" aria-selected={tab === id} onClick={() => setTab(id)}>
            {label}{n ? <span className="count num">{n}</span> : null}
          </button>
        ))}
      </nav>

      {!status && error && (
        <div className="banner" role="alert">
          Can’t reach the agent API (<code>{error}</code>). Start it with <code>flakenet --api-listen 127.0.0.1:7777 --dashboard-dir dashboard/web/dist</code>, or add <code>?demo</code> to the URL to preview sample data.
        </div>
      )}

      <main className="page">
        {status && tab === 'overview' && (
          <>
            <Hero d={status.deployment} s={status} />
            <div className="tiles" style={{ marginTop: 18 }}>
              <div className="tile"><div className="label">Peers online</div><div className="value num">{up.length}<span className="muted" style={{ fontSize: 18 }}> / {status.peers.length}</span></div><div className="sub">connected now</div></div>
              <div className="tile"><div className="label">CPU cores</div><div className="value num">{cores}</div><div className="sub">across online peers</div></div>
              <div className="tile"><div className="label">Free memory</div><div className="value num">{fmtKb(freeKb)}</div><div className="sub">across online peers</div></div>
              <div className="tile"><div className="label">Workloads</div><div className="value num">{status.workloads.length + (status.deployment ? 1 : 0)}</div><div className="sub">up {fmtUptime(status.uptime_secs)}</div></div>
            </div>
            <div className="section-title"><h2>Top peers</h2><button className="link" onClick={() => setTab('peers')}>View all</button></div>
            <div className="grid">{[...status.peers].sort((a, b) => (b.mem_available_kb ?? 0) - (a.mem_available_kb ?? 0)).slice(0, 4).map((p) => <PeerCard key={p.peer_id} p={p} />)}</div>
            <div className="section-title"><h2>Recent activity</h2><button className="link" onClick={() => setTab('activity')}>View all</button></div>
            <Feed events={status.events} limit={5} />
          </>
        )}

        {status && tab === 'peers' && (
          <>
            <div className="toolbar">
              {(['all', 'connected', 'trusted'] as Filter[]).map((f) => <button key={f} className="filter" aria-pressed={filter === f} onClick={() => setFilter(f)}>{f[0].toUpperCase() + f.slice(1)}</button>)}
              <select className="select" value={sort} onChange={(e) => setSort(e.target.value as Sort)} aria-label="Sort peers">
                <option value="memory">Most free memory</option><option value="cpu">Most CPU</option><option value="latency">Lowest latency</option>
              </select>
            </div>
            <div className="grid">
              <article className="card">
                <div className="media" style={{ background: `linear-gradient(135deg, ${peerColors(status.local_id)[0]}, ${peerColors(status.local_id)[1]})` }}>
                  <span className="badge"><i className="dot ok" />This node</span><Identicon id={status.local_id} />
                </div>
                <div className="card-body"><div className="card-title" title={status.local_id}>{shortId(status.local_id)}</div>
                  <div className="stats"><div className="stat"><div className="k">Uptime</div><div className="v num">{fmtUptime(status.uptime_secs)}</div></div><div className="stat"><div className="k">Version</div><div className="v num">v{status.version}</div></div></div>
                </div>
              </article>
              {peers.map((p) => <PeerCard key={p.peer_id} p={p} />)}
            </div>
            {peers.length === 0 && <div className="feed" style={{ marginTop: 18 }}><div className="empty">No peers match.</div></div>}
          </>
        )}

        {status && tab === 'deployments' && <Deployments s={status} q={q} />}
        {status && tab === 'activity' && <Feed events={events} />}
      </main>
    </>
  )
}
