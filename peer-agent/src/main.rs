use std::{collections::{HashMap, HashSet}, fs, io::Write, os::unix::fs::OpenOptionsExt, path::Path, time::Duration};

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use libp2p::{
    futures::StreamExt,
    identify, identity, kad,
    kad::store::MemoryStore,
    multiaddr::Protocol,
    ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId,
};
mod config;
mod deploy;
mod record;
mod scheduler;
mod tunnel;
use libp2p::{request_response, StreamProtocol};
use record::SignedRecord;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "flakenet", about = "FlakeNet: decentralized cloud peer agent")]
struct Args {
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// TOML config file (flags override it). Also read from $FLAKENET_CONFIG.
    #[arg(long)]
    config: Option<String>,
    /// Path to the Ed25519 identity key (created on first run, mode 0600).
    #[arg(long, global = true, default_value = "flakenet-peer.key")]
    key_file: String,
    /// Address to listen on.
    #[arg(long, default_value = "/ip4/0.0.0.0/udp/0/quic-v1")]
    listen: Multiaddr,
    /// Bootstrap peer multiaddr, ending in /p2p/<peer-id>. Repeatable.
    #[arg(long)]
    bootstrap: Vec<Multiaddr>,
    /// Peer IDs allowed to deploy workloads onto this agent. Repeatable.
    #[arg(long)]
    trust: Vec<PeerId>,
    /// Deploy this OCI image to the first peer we connect to.
    #[arg(long)]
    deploy_image: Option<String>,
    /// HOST:CONTAINER port mapping for --deploy-image.
    #[arg(long, default_value = "8080:80")]
    deploy_ports: String,
    /// Reject images not pinned by digest (name@sha256:...).
    #[arg(long)]
    require_digest: bool,
    /// Max concurrent workloads this agent will run.
    #[arg(long, default_value_t = 8)]
    max_workloads: usize,
    /// Host address workload ports bind to (default loopback; ingress fronts them).
    #[arg(long, default_value = "127.0.0.1")]
    workload_bind: String,
    /// Seconds to collect worker resource records before scheduling a deploy.
    #[arg(long, default_value_t = 3)]
    schedule_wait: u64,
    /// Seconds to wait for a worker's deploy response before rescheduling.
    #[arg(long, default_value_t = 120)]
    deploy_timeout: u64,
    /// After a successful deploy, expose the workload on this local address (e.g. 127.0.0.1:9000).
    #[arg(long)]
    gateway_listen: Option<std::net::SocketAddr>,
    /// Serve the gateway over TLS with this PEM certificate chain and key.
    #[arg(long, requires = "tls_key")]
    tls_cert: Option<String>,
    #[arg(long, requires = "tls_cert")]
    tls_key: Option<String>,
    /// Dev only: serve TLS with a generated self-signed cert for this domain.
    #[arg(long, conflicts_with = "tls_cert")]
    tls_self_signed: Option<String>,
    /// Max new gateway connections per source IP per 10 seconds.
    #[arg(long, default_value_t = 100)]
    rate_limit: u32,
    /// Registries workloads may be pulled from. Repeatable; default docker.io, ghcr.io, quay.io.
    #[arg(long = "allow-registry", default_values_t = ["docker.io".to_string(), "ghcr.io".to_string(), "quay.io".to_string()])]
    allow_registry: Vec<String>,
    /// Require images to carry a valid cosign signature for this public key (implies digest-pinning).
    #[arg(long)]
    cosign_key: Option<String>,
    /// Deployer: seconds between workload health probes (also refreshes resource records).
    #[arg(long, default_value_t = 5)]
    health_interval: u64,
    /// Deployer: consecutive failed probes before failing over to another worker.
    #[arg(long, default_value_t = 3)]
    health_failures: u32,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print this agent's peer ID (creates the key file if missing).
    Id,
    /// Write a starter config file.
    Init {
        #[arg(long, default_value = "flakenet.toml")]
        path: String,
    },
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kad: kad::Behaviour<MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
    stream: libp2p_stream::Behaviour,
    resources: request_response::json::Behaviour<(), SignedRecord>,
    deploy: request_response::json::Behaviour<deploy::DeployRequest, deploy::DeployResponse>,
}

fn load_or_create_identity(path: &str) -> Result<identity::Keypair> {
    let p = Path::new(path);
    if p.exists() {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(p)?.permissions().mode() & 0o077 != 0 {
            anyhow::bail!("key file {path} is accessible by group/others; chmod 600 it");
        }
        let bytes = fs::read(p).context("reading key file")?;
        return Ok(identity::Keypair::from_protobuf_encoding(&bytes)?);
    }
    let kp = identity::Keypair::generate_ed25519();
    // Created 0600 atomically (no window where the key is world-readable).
    let mut f = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(p)?;
    f.write_all(&kp.to_protobuf_encoding()?)?;
    Ok(kp)
}

#[tokio::main]
async fn main() -> Result<()> {
    let raw: Vec<String> = std::env::args().collect();
    let argv = match config::find_path(&raw) {
        Some(path) => {
            let src = fs::read_to_string(&path).with_context(|| format!("reading config {path}"))?;
            let known: HashSet<String> = Args::command().get_arguments()
                .filter_map(|a| a.get_long().map(String::from)).collect();
            config::merge(raw, &src, &known)?
        }
        None => raw,
    };
    let args = Args::parse_from(argv);
    match &args.cmd {
        Some(Cmd::Id) => {
            let kp = load_or_create_identity(&args.key_file)?;
            println!("{}", PeerId::from(kp.public()));
            return Ok(());
        }
        Some(Cmd::Init { path }) => {
            let mut f = fs::OpenOptions::new().write(true).create_new(true).open(path)
                .with_context(|| format!("{path} already exists or cannot be created"))?;
            f.write_all(config::TEMPLATE.as_bytes())?;
            println!("wrote {path}");
            return Ok(());
        }
        None => {}
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "flakenet=info".into()),
        )
        .init();
    let keypair = load_or_create_identity(&args.key_file)?;
    let dt = args.deploy_timeout;
    let local_id = PeerId::from(keypair.public());
    let signer = keypair.clone();
    let mut seq: u64 = 0;
    let mut running: Vec<String> = Vec::new();
    let mut inflight: usize = 0;
    let mut seen_nonces: HashSet<(PeerId, u64)> = HashSet::new();
    let policy = deploy::Policy {
        require_digest: args.require_digest,
        allowed_registries: args.allow_registry.clone(),
        cosign_key: args.cosign_key.clone(),
        min_nonce: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos() as u64,
        bind_addr: args.workload_bind.clone(),
    };
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<(
        request_response::ResponseChannel<deploy::DeployResponse>, PeerId, String, u16, anyhow::Result<String>)>();
    let pending_deploy = args.deploy_image.clone();
    let mut records: HashMap<PeerId, record::Body> = HashMap::new();
    // (image, fallback candidates best-first) for an in-progress scheduled deploy
    let mut job: Option<(String, Vec<PeerId>)> = None;
    let sched_timer = tokio::time::sleep(Duration::from_secs(args.schedule_wait));
    tokio::pin!(sched_timer);
    let mut sched_armed = pending_deploy.is_some();
    let mut current: Option<PeerId> = None;
    let mut fails: u32 = 0;
    let mut needs_failover = false;
    let mut bad: HashSet<PeerId> = HashSet::new();
    let mut target_tx: Option<tokio::sync::watch::Sender<(PeerId, u16)>> = None;
    let (health_tx, mut health_rx) = tokio::sync::mpsc::unbounded_channel::<bool>();
    let mut probe_inflight = false;
    let mut health = tokio::time::interval(Duration::from_secs(args.health_interval.max(1)));
    health.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let (hp, cp) = args.deploy_ports.split_once(':').context("--deploy-ports HOST:CONTAINER")?;
    let (hp, cp): (u16, u16) = (hp.parse()?, cp.parse()?);
    info!(%local_id, "FlakeNet agent identity");

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let id = key.public().to_peer_id();
            let mut kad = kad::Behaviour::new(id, MemoryStore::new(id));
            kad.set_mode(Some(kad::Mode::Server));
            Ok(Behaviour {
                kad,
                identify: identify::Behaviour::new(identify::Config::new(
                    "/flakenet/0.1.0".into(),
                    key.public(),
                )),
                ping: ping::Behaviour::default(),
                stream: libp2p_stream::Behaviour::new(),
                deploy: request_response::json::Behaviour::new(
                    [(StreamProtocol::new("/flakenet/deploy/1.0.0"), request_response::ProtocolSupport::Full)],
                    request_response::Config::default().with_request_timeout(Duration::from_secs(dt)),
                ),
                resources: request_response::json::Behaviour::new(
                    [(StreamProtocol::new("/flakenet/resources/1.0.0"), request_response::ProtocolSupport::Full)],
                    request_response::Config::default(),
                ),
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let allowed: tunnel::Allowed = Default::default();
    tokio::spawn(tunnel::serve(swarm.behaviour().stream.new_control(), args.trust.clone(), allowed.clone()));
    tokio::spawn(tunnel::serve_health(swarm.behaviour().stream.new_control(), args.trust.clone(), allowed.clone()));
    swarm.listen_on(args.listen.clone())?;

    for addr in args.bootstrap {
        let Some(Protocol::P2p(peer)) = addr.iter().last() else {
            warn!(%addr, "bootstrap address must end in /p2p/<peer-id>; skipping");
            continue;
        };
        swarm.behaviour_mut().kad.add_address(&peer, addr.clone());
        swarm.dial(addr)?;
    }

    loop {
        tokio::select! {
            ev = swarm.select_next_some() => match ev {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("listening on {address}/p2p/{local_id}");
                }
                SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    info!(%peer_id, addr = %endpoint.get_remote_address(), "connected");
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => info!(%peer_id, "disconnected"),
                SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. })) => {
                    for a in info.listen_addrs {
                        swarm.behaviour_mut().kad.add_address(&peer_id, a);
                    }
                    info!(%peer_id, agent = %info.protocol_version, "identified");
                    swarm.behaviour_mut().resources.send_request(&peer_id, ());
                }
                SwarmEvent::Behaviour(BehaviourEvent::Deploy(request_response::Event::OutboundFailure { peer, error, .. })) => {
                    warn!(%peer, "deploy request failed: {error}");
                    if let Some((img, rest)) = job.as_mut() {
                        if !rest.is_empty() {
                            let next = rest.remove(0);
                            warn!(%next, "rescheduling to next candidate");
                            let req = deploy::sign_request(&signer, img, hp, cp)?;
                            swarm.behaviour_mut().deploy.send_request(&next, req);
                        } else { warn!("no more candidates; deploy failed"); }
                    }
                }
                SwarmEvent::Behaviour(BehaviourEvent::Deploy(request_response::Event::Message { peer, message, .. })) => match message {
                    request_response::Message::Request { request, channel, .. } => {
                        let checked = deploy::authorize(&request, &peer, &args.trust).and_then(|b| {
                            deploy::check_policy(&b, &policy)?;
                            if running.len() + inflight >= args.max_workloads {
                                anyhow::bail!("workload limit reached ({})", args.max_workloads);
                            }
                            if !seen_nonces.insert((peer, b.nonce)) { anyhow::bail!("replayed request"); }
                            Ok(b)
                        });
                        match checked {
                            Ok(b) => {
                                inflight += 1;
                                let (tx, pol) = (done_tx.clone(), policy.clone());
                                let (img, port) = (b.image.clone(), b.host_port);
                                tokio::spawn(async move {
                                    let r = deploy::run(b, pol).await;
                                    let _ = tx.send((channel, peer, img, port, r));
                                });
                            }
                            Err(e) => {
                                warn!(%peer, "DEPLOY DENIED: {e:#}");
                                let _ = swarm.behaviour_mut().deploy.send_response(channel,
                                    deploy::DeployResponse { ok: false, message: format!("{e:#}") });
                            }
                        }
                    }
                    request_response::Message::Response { response, .. } => {
                        info!(%peer, ok = response.ok, msg = %response.message, "deploy result");
                        if !response.ok {
                            if let Some((img, rest)) = job.as_mut() {
                                if !rest.is_empty() {
                                    let next = rest.remove(0);
                                    warn!(%next, "rescheduling to next candidate");
                                    let req = deploy::sign_request(&signer, img, hp, cp)?;
                                    swarm.behaviour_mut().deploy.send_request(&next, req);
                                } else { warn!("no more candidates; deploy failed"); }
                            }
                        } else {
                            job = None;
                            current = Some(peer);
                            fails = 0;
                            if let Some(tx) = &target_tx {
                                info!(%peer, "gateway now routes to new worker");
                                tx.send_replace((peer, hp));
                            } else if let Some(addr) = args.gateway_listen {
                                let ctl = swarm.behaviour().stream.new_control();
                                let tls = match (&args.tls_cert, &args.tls_key, &args.tls_self_signed) {
                                    (Some(c), Some(k), _) => Some(tunnel::tls_config(&fs::read(c)?, &fs::read(k)?)?),
                                    (_, _, Some(d)) => Some(tunnel::self_signed(d)?),
                                    _ => None,
                                };
                                let opts = tunnel::GatewayOpts { tls, rate_per_10s: args.rate_limit };
                                let (tx, rx) = tokio::sync::watch::channel((peer, hp));
                                target_tx = Some(tx);
                                tokio::spawn(async move {
                                    if let Err(e) = tunnel::gateway(addr, ctl, rx, opts).await { warn!("gateway: {e:#}"); }
                                });
                            }
                        }
                    }
                },
                SwarmEvent::Behaviour(BehaviourEvent::Resources(request_response::Event::Message { peer, message, .. })) => match message {
                    request_response::Message::Request { channel, .. } => {
                        seq += 1;
                        match record::create(&signer, seq) {
                            Ok(rec) => { let _ = swarm.behaviour_mut().resources.send_response(channel, rec); }
                            Err(e) => warn!("cannot create record: {e}"),
                        }
                    }
                    request_response::Message::Response { response, .. } => match record::verify(&response, &peer) {
                        Ok(b) => {
                            info!(%peer, cpu = b.cpu_cores, mem_avail_kb = b.mem_available_kb, seq = b.seq, "VERIFIED resource record");
                            if records.len() < 1024 || records.contains_key(&peer) { records.insert(peer, b); }
                        }
                        Err(e) => warn!(%peer, "REJECTED resource record: {e:#}"),
                    },
                },
                SwarmEvent::Behaviour(BehaviourEvent::Ping(ping::Event { peer, result: Ok(rtt), .. })) => {
                    info!(%peer, ?rtt, "ping");
                }
                _ => {}
            },
            Some((channel, peer, img, port, res)) = done_rx.recv() => {
                inflight -= 1;
                let resp = match res {
                    Ok(name) => {
                        info!(%peer, image = %img, %name, port, "DEPLOYED workload");
                        running.push(name.clone());
                        allowed.lock().unwrap().insert(port);
                        deploy::DeployResponse { ok: true, message: name }
                    }
                    Err(e) => {
                        warn!(%peer, "DEPLOY FAILED: {e:#}");
                        deploy::DeployResponse { ok: false, message: format!("{e:#}") }
                    }
                };
                let _ = swarm.behaviour_mut().deploy.send_response(channel, resp);
            }
            _ = &mut sched_timer, if sched_armed => {
                sched_armed = false;
                let cands: Vec<(PeerId, record::Body)> = records.iter().filter(|(p, _)| swarm.is_connected(p)).map(|(p, b)| (*p, b.clone())).collect();
                let req = scheduler::Requirements { cpus: deploy::REQ_CPUS, memory_mb: deploy::REQ_MEM_MB };
                let mut ranked = scheduler::rank(&cands, &req);
                info!(candidates = cands.len(), eligible = ranked.len(), "scheduling");
                if ranked.is_empty() {
                    warn!("no eligible peer for deployment");
                } else {
                    let img = pending_deploy.clone().unwrap();
                    let first = ranked.remove(0);
                    info!(chosen = %first, %img, "SCHEDULED");
                    let r = deploy::sign_request(&signer, &img, hp, cp)?;
                    swarm.behaviour_mut().deploy.send_request(&first, r);
                    job = Some((img, ranked));
                }
            }
            _ = health.tick(), if pending_deploy.is_some() && !sched_armed => {
                // keep records fresh so failover decisions use live data
                let peers: Vec<PeerId> = swarm.connected_peers().cloned().collect();
                for p in peers { swarm.behaviour_mut().resources.send_request(&p, ()); }
                if needs_failover && job.is_none() {
                    let cands: Vec<(PeerId, record::Body)> = records.iter()
                        .filter(|(p, _)| swarm.is_connected(p) && !bad.contains(p))
                        .map(|(p, b)| (*p, b.clone())).collect();
                    let req = scheduler::Requirements { cpus: deploy::REQ_CPUS, memory_mb: deploy::REQ_MEM_MB };
                    let mut ranked = scheduler::rank(&cands, &req);
                    if ranked.is_empty() {
                        warn!("FAILOVER: no eligible peer; will retry");
                        bad.clear();
                    } else {
                        let img = pending_deploy.clone().unwrap();
                        let first = ranked.remove(0);
                        info!(chosen = %first, "FAILOVER: rescheduling workload");
                        let r = deploy::sign_request(&signer, &img, hp, cp)?;
                        swarm.behaviour_mut().deploy.send_request(&first, r);
                        job = Some((img, ranked));
                        needs_failover = false;
                    }
                } else if let (Some(p), false) = (current, probe_inflight) {
                    probe_inflight = true;
                    let (ctl, tx) = (swarm.behaviour().stream.new_control(), health_tx.clone());
                    tokio::spawn(async move { let _ = tx.send(tunnel::probe(ctl, p, hp).await); });
                }
            }
            Some(ok) = health_rx.recv() => {
                probe_inflight = false;
                if ok { fails = 0; } else if let Some(p) = current {
                    fails += 1;
                    warn!(worker = %p, fails, "health probe failed");
                    if fails >= args.health_failures {
                        warn!(worker = %p, "WORKER UNHEALTHY: failing over");
                        bad.insert(p);
                        current = None;
                        fails = 0;
                        needs_failover = true;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => { info!("shutting down"); for n in &running { deploy::remove(n); } break; }
        }
        
    }
    Ok(())
}
