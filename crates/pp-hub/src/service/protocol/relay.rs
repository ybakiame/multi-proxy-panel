use pp_common::{CoreType, PanelResult};
use pp_db::entities::{certificate, node, node_binding, protocol_config, relay_rule};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Relay rule integration
// ---------------------------------------------------------------------------

/// Match specification extracted from a relay rule's `match_config` for a
/// given core type.
#[derive(Debug)]
pub enum MatchSpec {
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
pub fn push_json(arr: &mut Value, item: Value) {
    if let Some(a) = arr.as_array_mut() {
        a.push(item);
    }
}

/// Extract match spec from a relay rule for a specific core type.
pub fn match_spec(rule: &relay_rule::Model, core_type: CoreType) -> Option<MatchSpec> {
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
pub fn str_list(v: Option<&Value>) -> Vec<String> {
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
pub fn apply_singbox_relay_rule(
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
pub fn apply_mihomo_relay_rule(
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
                insert(config, format!("DOMAIN,{}, {}", d, tag));
            }
            for s in suffixes {
                insert(config, format!("DOMAIN-SUFFIX,{}, {}", s, tag));
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
            insert(config, format!("RULE-SET,{}, {}", rs_tag, tag));
        }
        None => {}
    }
    Ok(())
}

/// Resolve the TLS SNI for a relay exit hop from the binding's tls override:
/// managed certificate domain first, then explicit domain.
pub async fn resolve_relay_sni(
    db: &DatabaseConnection,
    binding: &node_binding::Model,
) -> Option<String> {
    let tls = binding.override_settings.as_ref()?.get("tls_settings")?;
    if let Some(cert_id) = tls
        .get("cert_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        && let Ok(Some(cert)) = certificate::Entity::find_by_id(cert_id).one(db).await
    {
        return Some(cert.domain);
    }
    tls.get("domain")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Inject relay outbounds and route rules into a node's generated config.
pub async fn apply_relay_rules(
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
