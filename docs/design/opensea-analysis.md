# Design analysis: marketplace-style dashboard

Reference studied: the public OpenSea homepage (CSS tokens + page structure, fetched 2026-11-15).
Goal: give the FlakeNet dashboard the same *design language* — a dark, neutral, card-driven
marketplace UI — mapped onto compute peers and workloads instead of NFTs.

Only the design language is borrowed. We do **not** copy their code, images, icons, fonts
(GT America is a licensed typeface) or branding.

## Design tokens observed (dark theme)

| Role | Value |
|---|---|
| App background | `#080808` (pure neutral, no blue tint) |
| Surface / card | `#0c0c0c`, `#0f0f0f`, `#161616` |
| Raised / hover / input | `#202020`, `#333333` |
| Border | `#202020` (subtle), `#2c2c2c` (strong) |
| Text | primary `#ffffff`, secondary `#acacac`, tertiary `#444444` |
| Accent (CTA, links, focus) | `#0086ff` (hover `#2092ff`, tint `#83c3ff`) |
| Success / error / warning | `#47bb64` / `#e24756` / `#ffcc00` |
| Rarity accents | common grey, rare `#00a3ff`, epic `#d358ff`, legendary `#ff8a00` |

Light theme mirrors it: `#fafafa` app, `#ffffff` surface, `#171717` text, `#0086ff` accent.

- Radius scale: 2px / 4px / 6px / 8px / 12px (cards) / 16px / 24px; fully-round pills for buttons and chips.
- Spacing: 4px base unit. Top nav 64–68px. Filter toolbar 40px.
- Type: geometric grotesque sans for UI, monospace for numbers/ids. We substitute Inter + JetBrains Mono (open licences).
- Motion: subtle; hover lifts/brightens surfaces, no heavy animation.

## Layout patterns

1. **Sticky top nav** — logo, centred pill search, primary-blue action button, theme toggle.
2. **Horizontal category tabs** under the nav (Home / Collections / Tokens / Activity ...) with an underline on the active tab.
3. **Hero / featured carousel** — one large rounded card with a gradient.
4. **"Trending" ranked table** — rank, thumbnail, name, change %, sparkline, numeric columns, right-aligned tabular numbers.
5. **Card grid** — large rounded media area on top, name + verified badge, a two-column stat row, hover raises the card.
6. **Filter chips / pills** with counts, sort dropdown, grid/list toggle.
7. **Activity feed** — dense rows: icon, event, subject, time-ago.

## Mapping to FlakeNet

| Marketplace concept | FlakeNet concept |
|---|---|
| Collection / NFT card | Peer card (generated identicon from peer ID, CPU/RAM, ping, trust badge) |
| Floor price / volume | Free memory / cores / latency |
| Rarity tier | Trust tier (private / verified / community) |
| Trending table | Deployments table with health sparkline |
| Activity feed | Agent event log (scheduled, deployed, denied, failover) |
| Hero | Live deployment: image, worker, gateway URL, status |
| "Connect wallet" | Agent connection status / peer ID |

## Pages built

- **Overview** — stat tiles, live deployment hero, recent activity.
- **Peers** — searchable/filterable card grid.
- **Deployments** — table of workloads with status and health history.
- **Activity** — full event log.

Read-only for now: the dashboard displays what the agent reports. Mutating actions (deploy, kill) stay on the CLI
until the API has authentication and CSRF protection.
