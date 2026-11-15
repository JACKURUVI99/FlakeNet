//! Read-only snapshot of agent state, published to the HTTP API / dashboard.
//! Contains no secrets: peer IDs, public resource figures, workload names, event log.
use std::{collections::{HashMap, VecDeque}, time::{Instant, SystemTime, UNIX_EPOCH}};

use libp2p::PeerId;
use serde::Serialize;

use crate::record::Body;

#[derive(Serialize, Clone, Default)]
pub struct Status {
    pub local_id: String,
    pub version: &'static str,
    pub uptime_secs: u64,
    pub peers: Vec<PeerInfo>,
    pub workloads: Vec<WorkloadInfo>,
    pub deployment: Option<DeploymentInfo>,
    pub events: Vec<Event>,
}

#[derive(Serialize, Clone)]
pub struct PeerInfo {
    pub peer_id: String,
    pub connected: bool,
    pub trusted: bool,
    pub addr: Option<String>,
    pub rtt_ms: Option<f64>,
    pub cpu_cores: Option<u32>,
    pub mem_total_kb: Option<u64>,
    pub mem_available_kb: Option<u64>,
    pub record_expires_in_secs: Option<i64>,
}

#[derive(Serialize, Clone)]
pub struct WorkloadInfo {
    pub name: String,
    pub image: String,
    pub host_port: u16,
    pub started_unix: u64,
}

#[derive(Serialize, Clone)]
pub struct DeploymentInfo {
    pub image: String,
    pub state: &'static str,
    pub worker: Option<String>,
    pub host_port: u16,
    pub container_port: u16,
    pub failed_probes: u32,
    pub gateway: Option<String>,
    pub health: Vec<bool>,
}

#[derive(Serialize, Clone)]
pub struct Event {
    pub ts_unix: u64,
    pub level: &'static str,
    pub msg: String,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Append to the bounded event log.
pub fn note(events: &mut VecDeque<Event>, level: &'static str, msg: impl Into<String>) {
    if events.len() >= 200 { events.pop_front(); }
    events.push_back(Event { ts_unix: now(), level, msg: msg.into() });
}

pub struct Ctx<'a> {
    pub local_id: String,
    pub started: Instant,
    pub records: &'a HashMap<PeerId, Body>,
    pub connected: Vec<PeerId>,
    pub addrs: &'a HashMap<PeerId, String>,
    pub rtts: &'a HashMap<PeerId, f64>,
    pub trusted: &'a [PeerId],
    pub workloads: &'a [WorkloadInfo],
    pub deployment: Option<DeploymentInfo>,
    pub events: &'a VecDeque<Event>,
}

pub fn build(c: Ctx) -> Status {
    let mut ids: Vec<PeerId> = c.connected.clone();
    for p in c.records.keys() { if !ids.contains(p) { ids.push(*p); } }
    ids.sort_by_key(|p| p.to_string());
    let t = now() as i64;
    let peers = ids.iter().map(|p| {
        let rec = c.records.get(p);
        PeerInfo {
            peer_id: p.to_string(),
            connected: c.connected.contains(p),
            trusted: c.trusted.contains(p),
            addr: c.addrs.get(p).cloned(),
            rtt_ms: c.rtts.get(p).copied(),
            cpu_cores: rec.map(|b| b.cpu_cores),
            mem_total_kb: rec.map(|b| b.mem_total_kb),
            mem_available_kb: rec.map(|b| b.mem_available_kb),
            record_expires_in_secs: rec.map(|b| b.expires_unix as i64 - t),
        }
    }).collect();
    Status {
        local_id: c.local_id,
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: c.started.elapsed().as_secs(),
        peers,
        workloads: c.workloads.to_vec(),
        deployment: c.deployment,
        events: c.events.iter().rev().cloned().collect(), // newest first
    }
}
