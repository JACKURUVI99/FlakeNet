//! Config-file support: a TOML file is translated into CLI flags, so one code path
//! (clap) validates everything. Precedence: command line > config file > defaults.
use std::collections::HashSet;

use anyhow::{bail, Result};

pub const TEMPLATE: &str = r#"# FlakeNet agent configuration. Keys are the CLI flag names (kebab or snake case).
# Command-line flags override values here. Unknown keys are rejected.

key-file = "flakenet-peer.key"
listen = "/ip4/0.0.0.0/udp/4001/quic-v1"

# Peers to connect to at startup (multiaddr ending in /p2p/<peer-id>).
# bootstrap = ["/ip4/203.0.113.10/udp/4001/quic-v1/p2p/12D3Koo..."]

# Peer IDs allowed to deploy workloads onto this agent (get one with `flakenet id`).
# trust = ["12D3Koo..."]

# Worker policy
# allow-registry = ["docker.io", "ghcr.io"]
# require-digest = true
# cosign-key = "cosign.pub"
# max-workloads = 8

# Deployer: schedule an image onto a worker and expose it through a local gateway.
# deploy-image = "docker.io/library/nginx:alpine"
# deploy-ports = "8080:80"
# gateway-listen = "127.0.0.1:9000"
"#;

/// Find `--config PATH` / `--config=PATH` in argv, falling back to $FLAKENET_CONFIG.
pub fn find_path(argv: &[String]) -> Option<String> {
    let mut it = argv.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--config" { return it.next().cloned(); }
        if let Some(v) = a.strip_prefix("--config=") { return Some(v.to_string()); }
    }
    std::env::var("FLAKENET_CONFIG").ok().filter(|s| !s.is_empty())
}

/// Inject config values as flags right after argv[0], skipping any flag the user gave explicitly.
pub fn merge(argv: Vec<String>, src: &str, known: &HashSet<String>) -> Result<Vec<String>> {
    let table: toml::Table = src.parse().map_err(|e| anyhow::anyhow!("invalid config file: {e}"))?;
    let mut inject: Vec<String> = Vec::new();
    for (k, v) in table {
        let flag = k.replace('_', "-");
        if flag == "config" { continue; }
        if !known.contains(&flag) { bail!("unknown config key '{k}'"); }
        let long = format!("--{flag}");
        if argv.iter().any(|a| *a == long || a.starts_with(&format!("{long}="))) { continue; }
        let scalar = |v: &toml::Value| -> Result<String> {
            Ok(match v {
                toml::Value::String(s) => s.clone(),
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::Float(f) => f.to_string(),
                _ => bail!("config key '{k}' has an unsupported value type"),
            })
        };
        match &v {
            toml::Value::Boolean(true) => inject.push(long),
            toml::Value::Boolean(false) => {}
            // `--flag=value` form: a value can never be re-parsed as a separate flag.
            toml::Value::Array(items) => for it in items { inject.push(format!("{long}={}", scalar(it)?)); },
            other => inject.push(format!("{long}={}", scalar(other)?)),
        }
    }
    let mut out = Vec::with_capacity(argv.len() + inject.len());
    let mut argv = argv.into_iter();
    out.extend(argv.next());
    out.extend(inject);
    out.extend(argv);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> HashSet<String> {
        ["key-file", "trust", "require-digest", "max-workloads", "bootstrap"].iter().map(|s| s.to_string()).collect()
    }
    fn argv(a: &[&str]) -> Vec<String> { a.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn injects_scalars_bools_arrays() {
        let out = merge(argv(&["flakenet"]), "key_file='k'\nrequire-digest=true\nmax-workloads=3\ntrust=['a','b']", &known()).unwrap();
        let joined = out.join(" ");
        assert!(joined.contains("--key-file=k") && joined.contains("--require-digest"));
        assert!(joined.contains("--max-workloads=3") && joined.contains("--trust=a --trust=b"));
    }

    #[test]
    fn cli_overrides_config() {
        let out = merge(argv(&["flakenet", "--max-workloads=9", "--trust", "x"]), "max-workloads=3\ntrust=['a']", &known()).unwrap();
        assert!(!out.iter().any(|a| a == "--max-workloads=3" || a == "--trust=a"));
    }

    #[test]
    fn unknown_key_rejected() {
        assert!(merge(argv(&["flakenet"]), "trsut=['a']", &known()).is_err());
    }

    #[test]
    fn false_bool_and_bad_types() {
        let out = merge(argv(&["flakenet"]), "require-digest=false", &known()).unwrap();
        assert_eq!(out, argv(&["flakenet"]));
        assert!(merge(argv(&["flakenet"]), "key-file={a=1}", &known()).is_err());
        assert!(merge(argv(&["flakenet"]), "this is not toml", &known()).is_err());
    }

    #[test]
    fn flags_stay_before_subcommand() {
        let out = merge(argv(&["flakenet", "id"]), "max-workloads=3", &known()).unwrap();
        assert_eq!(out, argv(&["flakenet", "--max-workloads=3", "id"]));
    }

    #[test]
    fn flag_like_value_cannot_inject_another_flag() {
        let out = merge(argv(&["flakenet"]), "key-file = '--trust=12D3Evil'", &known()).unwrap();
        assert_eq!(out, argv(&["flakenet", "--key-file=--trust=12D3Evil"])); // one argv entry, one flag
    }

    #[test]
    fn template_parses_and_uses_known_keys() {
        let k: HashSet<String> = ["key-file", "listen"].iter().map(|s| s.to_string()).collect();
        assert!(merge(argv(&["flakenet"]), TEMPLATE, &k).is_ok());
    }
}
