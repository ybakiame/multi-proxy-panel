use pp_common::{CoreType, PanelResult};
use pp_config::{BuilderRegistry, InboundConfig};
use pp_db::entities::{
    certificate, client, client_group_binding, core_version, node, node_binding,
    node_binding_group_binding, protocol_config, relay_rule,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use std::collections::HashSet;
use uuid::Uuid;

/// Generate core configuration for a specific node based on its bindings.
/// Returns the config JSON and the effective core binary version (for sing-box
/// only) derived from the active protocol configs.
pub async fn generate_node_config(
    db: &DatabaseConnection,
    node_id: Uuid,
    target_core: CoreType,
) -> PanelResult<(Value, Option<String>)> {
    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::NodeId.eq(node_id))
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    let mut inbounds = Vec::new();

    for binding in bindings {
        let config = protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await?
            .ok_or_else(|| {
                pp_common::PanelError::NotFound(format!(
                    "protocol config {} not found",
                    binding.protocol_config_id
                ))
            })?;

        // Check if this config applies to the target core
        let config_core = parse_core_type(&config.core_type);
        if config_core != target_core {
            continue;
        }

        let protocol = parse_protocol_type(&config.protocol_type)?;
        let mut settings = config.settings.clone();

        // Merge override settings from binding, keeping tls_settings separate.
        let override_tls = binding
            .override_settings
            .as_ref()
            .and_then(|o| o.get("tls_settings").cloned());
        if let Some(ref overrides) = binding.override_settings {
            if let Some(obj) = settings.as_object_mut() {
                if let Some(over_obj) = overrides.as_object() {
                    for (k, v) in over_obj {
                        if k == "tls_settings" {
                            continue;
                        }
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        // Inject clients bound to this binding through shared groups.
        inject_binding_clients(db, &binding, &config.protocol_type, &mut settings).await?;

        // Builders read port/listen/tag from settings, so merge InboundConfig fields.
        if let Some(obj) = settings.as_object_mut() {
            obj.insert("port".to_string(), json!(config.listen_port));
            obj.insert("listen".to_string(), json!(config.listen_address.clone()));
            obj.insert(
                "tag".to_string(),
                json!(format!("{}-{}", config.name, config.id)),
            );
        }

        let effective_tls = pp_common::settings_helper::merge_tls_settings(
            config.tls_settings.clone(),
            override_tls,
        );
        let effective_tls = resolve_managed_cert_tls(db, node_id, effective_tls).await?;

        inbounds.push(InboundConfig {
            tag: format!("{}-{}", config.name, config.id),
            protocol,
            listen: config.listen_address.clone(),
            port: config.listen_port as u16,
            settings,
            tls: effective_tls,
            sniffing: None,
            core_version: config.core_version.clone(),
        });
    }

    let effective_version = effective_core_version(target_core, &inbounds);

    let registry = BuilderRegistry::default();
    let builder = registry.get(target_core).ok_or_else(|| {
        pp_common::PanelError::Config(format!("no builder registered for {:?}", target_core))
    })?;

    let mut config = builder.build_full_config(&inbounds)?;
    apply_relay_rules(db, node_id, target_core, &mut config).await?;
    Ok((config, effective_version))
}

/// Translate a managed-certificate TLS reference (`{"cert_id": ...}`) into
/// the agent-side unified layout (`certs/<domain>.{crt,key}`). The
/// certificate must belong to the node the config is generated for.
async fn resolve_managed_cert_tls(
    db: &DatabaseConnection,
    node_id: Uuid,
    tls: Option<Value>,
) -> PanelResult<Option<Value>> {
    let Some(tls) = tls else {
        return Ok(None);
    };
    let Some(cert_id_raw) = tls.get("cert_id").and_then(|v| v.as_str()) else {
        return Ok(Some(tls));
    };

    let cert_id = Uuid::parse_str(cert_id_raw)
        .map_err(|_| pp_common::PanelError::Validation("invalid cert_id in tls_settings".into()))?;
    let cert = pp_db::entities::certificate::Entity::find_by_id(cert_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            pp_common::PanelError::NotFound(format!("certificate {} not found", cert_id))
        })?;
    if cert.node_id != node_id {
        return Err(pp_common::PanelError::Validation(format!(
            "certificate {} ({}) does not belong to this node",
            cert_id, cert.domain
        )));
    }

    Ok(Some(json!({ "managed_domain": cert.domain })))
}

/// Find active clients that share at least one group with a node binding and
/// inject them as a `clients` array into the protocol settings.
async fn inject_binding_clients(
    db: &DatabaseConnection,
    binding: &node_binding::Model,
    protocol_type: &str,
    settings: &mut Value,
) -> PanelResult<()> {
    let binding_group_ids = node_binding_group_binding::Entity::find()
        .filter(node_binding_group_binding::Column::NodeBindingId.eq(binding.id))
        .all(db)
        .await?
        .into_iter()
        .map(|g| g.group_id)
        .collect::<Vec<_>>();

    if binding_group_ids.is_empty() {
        // No groups on the binding means no clients are authorized.
        return Ok(());
    }

    let group_set: HashSet<Uuid> = binding_group_ids.into_iter().collect();

    // Find active clients whose group memberships overlap with the binding groups.
    let client_bindings = client_group_binding::Entity::find()
        .filter(client_group_binding::Column::GroupId.is_in(group_set.iter().cloned()))
        .all(db)
        .await?;

    let client_ids: HashSet<Uuid> = client_bindings.into_iter().map(|b| b.client_id).collect();

    if client_ids.is_empty() {
        return Ok(());
    }

    let clients = client::Entity::find()
        .filter(client::Column::Id.is_in(client_ids.iter().cloned()))
        .filter(client::Column::Status.eq("active"))
        .all(db)
        .await?;

    let clients_json: Vec<Value> = clients
        .iter()
        .map(|c| client_to_protocol_entry(c, protocol_type))
        .collect();

    if let Some(obj) = settings.as_object_mut() {
        obj.insert("clients".to_string(), json!(clients_json));
    }

    Ok(())
}

/// Map a client to the protocol-specific client entry expected by pp-config builders.
///
/// The injected identifier (vless `email`, password-protocol `name`) is what the
/// core reports back as the connection user, so it must stay resolvable by the
/// hub: fall back to the client UUID when no email is set.
fn client_to_protocol_entry(client: &client::Model, protocol_type: &str) -> Value {
    let fallback = client.id.to_string();
    let email = client.email.as_ref().unwrap_or(&fallback);
    match protocol_type {
        pt if pt.starts_with("vless") => {
            let flow = if pt == "vless_reality" {
                "xtls-rprx-vision"
            } else {
                ""
            };
            let mut obj = json!({
                "id": client.id.to_string(),
                "email": email,
                "flow": flow,
            });
            if let Some(limit) = client.max_devices {
                if limit > 0 {
                    if let Some(map) = obj.as_object_mut() {
                        map.insert("limitIp".to_string(), json!(limit));
                    }
                }
            }
            obj
        }
        "hysteria2" | "anytls" => json!({
            "name": email,
            "password": client.id.to_string(),
        }),
        _ => json!({"id": client.id.to_string()}),
    }
}

/// Content-hash config version (first 16 hex chars of the SHA-256 of the
/// serialized config). Deterministic: identical config -> identical version.
pub fn config_version_of(config_str: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(config_str.as_bytes());
    hex::encode(&digest[..8])
}

/// Push generated core config to the agent running on a node.
/// Returns Ok(()) on successful delivery, or an error describing the failure.
pub async fn push_node_config(
    state: &crate::state::AppState,
    node_id: Uuid,
    core_type: CoreType,
    restart: bool,
    version: Option<String>,
) -> PanelResult<()> {
    validate_node_port_conflicts(&state.db, node_id)
        .await
        .map_err(|e| {
            pp_common::PanelError::Validation(format!("node {} port conflict: {}", node_id, e))
        })?;

    let (config_json, core_version) = generate_node_config(&state.db, node_id, core_type).await?;
    let config_str = serde_json::to_string(&config_json)
        .map_err(|e| pp_common::PanelError::Config(format!("failed to serialize config: {e}")))?;

    let build_id = core_build_id_of(&state.db, core_type, core_version.as_deref()).await;
    let version = version
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| push_version_of(&config_str, &build_id));

    let proto_core = match core_type {
        CoreType::SingBox => pp_proto::CoreType::SingBox,
        CoreType::Mihomo => pp_proto::CoreType::Mihomo,
    };

    let message = pp_proto::HubMessage {
        payload: Some(pp_proto::hub_message::Payload::ConfigPush(
            pp_proto::ConfigPush {
                config_json: config_str,
                target_core: proto_core as i32,
                restart_required: restart,
                config_version: version.clone(),
                core_version: core_version.unwrap_or_default(),
                core_build_id: build_id,
            },
        )),
    };

    state
        .send_to_agent(node_id, message)
        .await
        .map_err(|e| pp_common::PanelError::Internal(format!("failed to push config: {e}")))?;

    state
        .set_agent_config_version(&node_id, &core_type.to_string(), version)
        .await;

    Ok(())
}

/// Find all node IDs that have an active binding to a given protocol config.
pub async fn nodes_using_config(
    db: &DatabaseConnection,
    config_id: Uuid,
) -> PanelResult<Vec<Uuid>> {
    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::ProtocolConfigId.eq(config_id))
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    let node_ids: HashSet<Uuid> = bindings.into_iter().map(|b| b.node_id).collect();
    Ok(node_ids.into_iter().collect())
}
/// Resolve the effective pinned core version across inbounds.
///
/// sing-box: highest explicitly requested version, or a stable default of
/// v1.13.14 (set a protocol config's core_version to a 1.14.0 alpha tag such
/// as `v1.14.0-alpha.43` for the new gRPC API service).
/// mihomo: highest explicitly requested version, or None (= latest upstream).
fn effective_core_version(core_type: CoreType, inbounds: &[InboundConfig]) -> Option<String> {
    let requested: Vec<&str> = inbounds
        .iter()
        .filter_map(|i| i.core_version.as_deref())
        .filter(|v| !v.is_empty())
        .collect();

    match core_type {
        CoreType::SingBox => {
            if requested.is_empty() {
                return Some("v1.13.14".to_string());
            }
            requested
                .into_iter()
                .max_by(|a, b| compare_versions(a, b))
                .map(|v| v.to_string())
        }
        CoreType::Mihomo => requested
            .into_iter()
            .max_by(|a, b| compare_versions(a, b))
            .map(|v| v.to_string()),
    }
}

/// Build identifier for a pinned core version: the upstream publish time
/// (Unix seconds) recorded in the version catalog. Rolling tags keep the
/// same version string across builds, so this is what actually tells builds
/// apart. Empty when the version is unpinned or has no metadata recorded.
pub async fn core_build_id_of(
    db: &DatabaseConnection,
    core_type: CoreType,
    version: Option<&str>,
) -> String {
    let Some(version) = version.filter(|v| !v.is_empty()) else {
        return String::new();
    };
    core_version::Entity::find()
        .filter(core_version::Column::CoreType.eq(core_type.to_string()))
        .filter(core_version::Column::Version.eq(version))
        .one(db)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.published_at)
        .map(|t| t.timestamp().to_string())
        .unwrap_or_default()
}

/// Config version that also changes when the upstream build of a pinned
/// rolling tag changes, so nodes pick up rebuilt binaries.
pub fn push_version_of(config_str: &str, build_id: &str) -> String {
    if build_id.is_empty() {
        config_version_of(config_str)
    } else {
        config_version_of(&format!("{}#build:{}", config_str, build_id))
    }
}

/// Simple semver-like comparison. Returns `Ordering` for two version strings.
/// Pre-release segments (e.g. `-beta.5`) are treated as lower than release.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse(v: &str) -> Vec<u32> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect()
    }
    parse(a).cmp(&parse(b))
}

/// Validate that active bindings on a node do not ask two cores to listen on the
/// same address/port. Returns an error if any overlap is found.
pub async fn validate_node_port_conflicts(
    db: &DatabaseConnection,
    node_id: Uuid,
) -> PanelResult<()> {
    use std::collections::{HashMap, HashSet};

    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::NodeId.eq(node_id))
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    // Map (listen, port) -> set of cores that want it.
    let mut port_cores: HashMap<(String, u16), HashSet<CoreType>> = HashMap::new();

    for binding in bindings {
        let config = match protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await?
        {
            Some(c) => c,
            None => continue,
        };

        let mut listen = config.listen_address.clone();
        let mut port = config.listen_port as u16;

        // Apply binding-level overrides for listen/port if present.
        if let Some(overrides) = binding.override_settings {
            if let Some(obj) = overrides.as_object() {
                if let Some(v) = obj.get("listen_address").and_then(|v| v.as_str()) {
                    listen = v.to_string();
                }
                if let Some(v) = obj.get("listen_port").and_then(|v| v.as_u64()) {
                    port = v as u16;
                }
            }
        }

        let config_core = parse_core_type(&config.core_type);
        let cores: Vec<CoreType> = vec![config_core];

        let entry = port_cores.entry((listen.clone(), port)).or_default();
        for c in cores {
            if !entry.insert(c) {
                return Err(pp_common::PanelError::Validation(format!(
                    "port conflict on {}:{}: multiple inbounds target {:?}",
                    listen, port, c
                )));
            }
        }
    }

    Ok(())
}

// --- Relay rule integration ---

/// Match specification extracted from a relay rule's `match_config` for a
/// given core type.
#[derive(Debug)]
enum MatchSpec {
    Inline {
        domains: Vec<String>,
        suffixes: Vec<String>,
    },
    RuleSet {
        url: String,
        extra: String, // format (sing-box) or behavior (mihomo)
    },
}

/// Push a JSON value into an array, creating the array if needed.
fn push_json(arr: &mut Value, item: Value) {
    if let Some(a) = arr.as_array_mut() {
        a.push(item);
    }
}

/// Extract match spec from a relay rule for a specific core type.
fn match_spec(rule: &relay_rule::Model, core_type: CoreType) -> Option<MatchSpec> {
    let cfg = &rule.match_config;
    if rule.match_type == "inline" {
        let domains = str_list(cfg.get("domains"));
        let suffixes = str_list(cfg.get("domain_suffixes"));
        if domains.is_empty() && suffixes.is_empty() {
            return None;
        }
        Some(MatchSpec::Inline { domains, suffixes })
    } else {
        // rule_set
        if let Some(lib) = cfg.get("library").and_then(|v| v.as_str()) {
            let entry = crate::routes::relay_rule::library_lookup(lib)?;
            return Some(match core_type {
                CoreType::SingBox => MatchSpec::RuleSet {
                    url: entry.1.to_string(),
                    extra: entry.2.to_string(),
                },
                CoreType::Mihomo => MatchSpec::RuleSet {
                    url: entry.3.to_string(),
                    extra: entry.4.to_string(),
                },
            });
        }
        let custom = cfg.get("custom")?;
        let (key_url, key_extra) = match core_type {
            CoreType::SingBox => ("singbox", "format"),
            CoreType::Mihomo => ("mihomo", "behavior"),
        };
        let url = custom
            .get(key_url)
            .and_then(|c| c.get("url"))
            .and_then(|v| v.as_str())?
            .to_string();
        let extra = custom
            .get(key_url)
            .and_then(|c| c.get(key_extra))
            .and_then(|v| v.as_str())
            .unwrap_or(match core_type {
                CoreType::SingBox => "binary",
                CoreType::Mihomo => "domain",
            })
            .to_string();
        Some(MatchSpec::RuleSet { url, extra })
    }
}

/// Extract non-empty string values from an optional JSON array.
fn str_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Apply a sing-box relay rule: inject an outbound rule entry and a rule-set
/// reference (if applicable) into the config.
fn apply_singbox_relay_rule(
    config: &mut Value,
    tag: &str,
    rs_tag: &str,
    rule: &relay_rule::Model,
) -> PanelResult<()> {
    match match_spec(rule, CoreType::SingBox) {
        Some(MatchSpec::Inline { domains, suffixes }) => {
            let mut r = json!({ "outbound": tag });
            if !domains.is_empty() {
                r["domain"] = json!(domains);
            }
            if !suffixes.is_empty() {
                r["domain_suffix"] = json!(suffixes);
            }
            push_json(&mut config["route"]["rules"], r);
        }
        Some(MatchSpec::RuleSet { url, extra }) => {
            if !config["route"]
                .get("rule_set")
                .is_some_and(|v| v.is_array())
            {
                config["route"]["rule_set"] = json!([]);
            }
            push_json(
                &mut config["route"]["rule_set"],
                json!({
                    "type": "remote",
                    "tag": rs_tag,
                    "format": extra,
                    "url": url,
                }),
            );
            push_json(
                &mut config["route"]["rules"],
                json!({ "rule_set": [rs_tag], "outbound": tag }),
            );
        }
        None => {}
    }
    Ok(())
}

/// Apply a mihomo relay rule: inject a proxy group entry and a rule-provider
/// (if applicable) into the config.
fn apply_mihomo_relay_rule(
    config: &mut Value,
    tag: &str,
    rs_tag: &str,
    rule: &relay_rule::Model,
) -> PanelResult<()> {
    // mihomo rules end with "MATCH,DIRECT"; insert relay rules before it.
    let insert = |config: &mut Value, entry: String| {
        if let Some(rules) = config["rules"].as_array_mut() {
            let pos = rules
                .iter()
                .position(|r| r.as_str().is_some_and(|s| s.starts_with("MATCH")))
                .unwrap_or(rules.len());
            rules.insert(pos, json!(entry));
        }
    };
    match match_spec(rule, CoreType::Mihomo) {
        Some(MatchSpec::Inline { domains, suffixes }) => {
            for d in domains {
                insert(config, format!("DOMAIN,{},{}", d, tag));
            }
            for s in suffixes {
                insert(config, format!("DOMAIN-SUFFIX,{},{}", s, tag));
            }
        }
        Some(MatchSpec::RuleSet { url, extra }) => {
            if !config.get("rule-providers").is_some_and(|v| v.is_object()) {
                config["rule-providers"] = json!({});
            }
            config["rule-providers"][rs_tag] = json!({
                "type": "http",
                "behavior": extra,
                "format": "yaml",
                "url": url,
                "path": format!("./ruleset/{}.yaml", rs_tag),
            });
            insert(config, format!("RULE-SET,{},{}", rs_tag, tag));
        }
        None => {}
    }
    Ok(())
}

/// Resolve the TLS SNI for a relay exit hop from the binding's tls override:
/// managed certificate domain first, then explicit domain.
async fn resolve_relay_sni(
    db: &DatabaseConnection,
    binding: &node_binding::Model,
) -> Option<String> {
    let tls = binding.override_settings.as_ref()?.get("tls_settings")?;
    if let Some(cert_id) = tls
        .get("cert_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        if let Ok(Some(cert)) = certificate::Entity::find_by_id(cert_id).one(db).await {
            return Some(cert.domain);
        }
    }
    tls.get("domain")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Inject relay outbounds and route rules into a node's generated config.
async fn apply_relay_rules(
    db: &DatabaseConnection,
    node_id: Uuid,
    core_type: CoreType,
    config: &mut Value,
) -> PanelResult<()> {
    let rules = relay_rule::Entity::find()
        .filter(relay_rule::Column::NodeId.eq(node_id))
        .filter(relay_rule::Column::Enabled.eq(true))
        .all(db)
        .await
        .map_err(pp_common::PanelError::Database)?;
    if rules.is_empty() {
        return Ok(());
    }

    let db_err = |e: sea_orm::DbErr| pp_common::PanelError::Database(e);

    for rule in rules {
        let tag = format!("relay-{}", &rule.id.to_string()[..8]);
        let rs_tag = format!("rs-{}", &rule.id.to_string()[..8]);

        // Resolve exit hop
        let Some(binding) = node_binding::Entity::find_by_id(rule.exit_binding_id)
            .one(db)
            .await
            .map_err(&db_err)?
        else {
            tracing::warn!("relay rule {}: exit binding missing, skipping", rule.id);
            continue;
        };
        let Some(exit_cfg) = protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await
            .map_err(&db_err)?
        else {
            continue;
        };
        let Some(exit_node) = node::Entity::find_by_id(binding.node_id)
            .one(db)
            .await
            .map_err(&db_err)?
        else {
            continue;
        };

        let protocol = match exit_cfg.protocol_type.parse::<pp_common::ProtocolType>() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!(
                    "relay rule {}: bad protocol {}",
                    rule.id,
                    exit_cfg.protocol_type
                );
                continue;
            }
        };
        let server = exit_node
            .domain
            .clone()
            .unwrap_or_else(|| exit_node.address.clone());
        let credential = rule.relay_client_id.to_string();

        // TLS SNI for hysteria2/anytls: prefer managed certificate domain
        // from the binding's tls override, else the tls domain field.
        let tls_sni = resolve_relay_sni(db, &binding).await;

        let hop = pp_config::RelayHop {
            tag: &tag,
            protocol,
            settings: &exit_cfg.settings,
            server: &server,
            port: exit_cfg.listen_port as u16,
            credential: &credential,
            tls_sni: tls_sni.as_deref(),
        };

        match core_type {
            CoreType::SingBox => {
                let outbound = pp_config::build_singbox_outbound(&hop).map_err(|e| {
                    tracing::warn!("relay rule {}: {}", rule.id, e);
                    e
                })?;
                push_json(&mut config["outbounds"], outbound);
                apply_singbox_relay_rule(config, &tag, &rs_tag, &rule)?;
            }
            CoreType::Mihomo => {
                let proxy = pp_config::build_mihomo_proxy(&hop).map_err(|e| {
                    tracing::warn!("relay rule {}: {}", rule.id, e);
                    e
                })?;
                if !config.get("proxies").is_some_and(|v| v.is_array()) {
                    config["proxies"] = json!([]);
                }
                push_json(&mut config["proxies"], proxy);
                apply_mihomo_relay_rule(config, &tag, &rs_tag, &rule)?;
            }
        }
    }
    Ok(())
}

fn parse_core_type(s: &str) -> CoreType {
    match s {
        "sing-box" | "singbox" => CoreType::SingBox,
        "mihomo" => CoreType::Mihomo,
        _ => CoreType::SingBox,
    }
}

fn parse_protocol_type(s: &str) -> PanelResult<pp_common::ProtocolType> {
    use pp_common::ProtocolType;
    match s {
        "vless_reality" => Ok(ProtocolType::VlessReality),
        "vless_xhttp" => Ok(ProtocolType::VlessXhttp),
        "hysteria2" => Ok(ProtocolType::Hysteria2),
        "anytls" => Ok(ProtocolType::Anytls),
        _ => Err(pp_common::PanelError::Validation(format!(
            "unknown protocol type: {}",
            s
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client(id: Uuid, name: &str, email: Option<&str>) -> client::Model {
        client::Model {
            id,
            user_id: Uuid::new_v4(),
            name: name.to_string(),
            email: email.map(str::to_string),
            traffic_limit_bytes: 0,
            traffic_used_bytes: 0,
            all_time_used_bytes: 0,
            expiry_date: None,
            reset_day: None,
            data_limit_reset_strategy: "no_reset".to_string(),
            last_traffic_reset_time: None,
            max_devices: None,
            status: "active".to_string(),
            on_hold_expire_duration_secs: None,
            on_hold_timeout: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn vless_entry_uses_email_when_set() {
        let id = Uuid::new_v4();
        let entry = client_to_protocol_entry(
            &test_client(id, "alice", Some("alice@example.com")),
            "vless_reality",
        );
        assert_eq!(entry["id"], id.to_string());
        assert_eq!(entry["email"], "alice@example.com");
        assert_eq!(entry["flow"], "xtls-rprx-vision");
    }

    #[test]
    fn vless_entry_falls_back_to_uuid_without_email() {
        let id = Uuid::new_v4();
        let entry = client_to_protocol_entry(&test_client(id, "ybakiame", None), "vless_reality");
        assert_eq!(entry["email"], id.to_string());
    }

    #[test]
    fn hysteria2_entry_falls_back_to_uuid_without_email() {
        let id = Uuid::new_v4();
        let entry = client_to_protocol_entry(&test_client(id, "ybakiame", None), "hysteria2");
        assert_eq!(entry["name"], id.to_string());
        assert_eq!(entry["password"], id.to_string());
    }

    #[test]
    fn anytls_entry_uses_email_as_name_when_set() {
        let id = Uuid::new_v4();
        let entry = client_to_protocol_entry(
            &test_client(id, "alice", Some("alice@example.com")),
            "anytls",
        );
        assert_eq!(entry["name"], "alice@example.com");
        assert_eq!(entry["password"], id.to_string());
    }

    // --- Relay rule helpers ---

    fn test_relay_rule(match_type: &str, match_config: Value) -> relay_rule::Model {
        relay_rule::Model {
            id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            exit_binding_id: Uuid::new_v4(),
            relay_client_id: Uuid::new_v4(),
            name: "test-rule".to_string(),
            match_type: match_type.to_string(),
            match_config,
            enabled: true,
            sort_order: 0,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn match_spec_inline_with_domains() {
        let rule = test_relay_rule(
            "inline",
            json!({
                "domains": ["example.com", "test.com"],
                "domain_suffixes": ["example.org"],
            }),
        );
        let spec = match_spec(&rule, CoreType::SingBox);
        match spec {
            Some(MatchSpec::Inline { domains, suffixes }) => {
                assert_eq!(domains, vec!["example.com", "test.com"]);
                assert_eq!(suffixes, vec!["example.org"]);
            }
            other => panic!("expected Inline, got {:?}", other),
        }
    }

    #[test]
    fn match_spec_rule_set_netflix_singbox() {
        let rule = test_relay_rule(
            "rule_set",
            json!({
                "library": "netflix",
            }),
        );
        let spec = match_spec(&rule, CoreType::SingBox).expect("expected Some");
        match spec {
            MatchSpec::RuleSet { url, extra } => {
                assert!(url.ends_with("geosite-netflix.srs"), "url: {}", url);
                assert_eq!(extra, "binary");
            }
            other => panic!("expected RuleSet, got {:?}", other),
        }
    }

    #[test]
    fn match_spec_rule_set_netflix_mihomo() {
        let rule = test_relay_rule(
            "rule_set",
            json!({
                "library": "netflix",
            }),
        );
        let spec = match_spec(&rule, CoreType::Mihomo).expect("expected Some");
        match spec {
            MatchSpec::RuleSet { url, extra } => {
                assert!(url.ends_with("netflix.yaml"), "url: {}", url);
                assert_eq!(extra, "classical");
            }
            other => panic!("expected RuleSet, got {:?}", other),
        }
    }

    #[test]
    fn match_spec_custom_per_core_urls() {
        let rule = test_relay_rule(
            "rule_set",
            json!({
                "custom": {
                    "singbox": {
                        "url": "https://example.com/custom.srs",
                        "format": "binary",
                    },
                    "mihomo": {
                        "url": "https://example.com/custom.yaml",
                        "behavior": "domain",
                    },
                }
            }),
        );
        // sing-box
        let spec_sb = match_spec(&rule, CoreType::SingBox).expect("expected Some");
        match spec_sb {
            MatchSpec::RuleSet { url, extra } => {
                assert_eq!(url, "https://example.com/custom.srs");
                assert_eq!(extra, "binary");
            }
            other => panic!("expected RuleSet, got {:?}", other),
        }
        // mihomo
        let spec_mh = match_spec(&rule, CoreType::Mihomo).expect("expected Some");
        match spec_mh {
            MatchSpec::RuleSet { url, extra } => {
                assert_eq!(url, "https://example.com/custom.yaml");
                assert_eq!(extra, "domain");
            }
            other => panic!("expected RuleSet, got {:?}", other),
        }
    }

    #[test]
    fn match_spec_empty_inline_returns_none() {
        let rule = test_relay_rule(
            "inline",
            json!({
                "domains": [],
                "domain_suffixes": [],
            }),
        );
        assert!(match_spec(&rule, CoreType::SingBox).is_none());
    }
}
