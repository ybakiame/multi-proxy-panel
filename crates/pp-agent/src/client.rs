use pp_proto::{
    AgentMessage, ConfigPush, CoreCommand, Heartbeat, HostMetrics, HubMessage, OnlineUser,
    OnlineUsersReport, RegisterRequest, hub_agent_client::HubAgentClient,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;

use pp_core::CoreSupervisor;

use crate::logger::{AgentLogger, collect_core_status, collect_logs};
use crate::persist;

pub struct AgentStreamClient {
    agent_id: String,
    token: String,
    hostname: String,
    domain: String,
    tls_config: Option<tonic::transport::ClientTlsConfig>,
    data_dir: PathBuf,
    #[allow(dead_code)]
    hub_tx: mpsc::Sender<AgentMessage>,
    #[allow(dead_code)]
    hub_rx: Option<mpsc::Receiver<HubMessage>>,
    logger: AgentLogger,
}

impl AgentStreamClient {
    pub fn new(
        agent_id: String,
        token: String,
        hostname: String,
        domain: String,
        tls_config: Option<tonic::transport::ClientTlsConfig>,
        logger: AgentLogger,
        data_dir: PathBuf,
    ) -> Self {
        let (hub_tx, _hub_rx) = mpsc::channel::<AgentMessage>(128);
        Self {
            agent_id,
            token,
            hostname,
            domain,
            tls_config,
            data_dir,
            hub_tx,
            hub_rx: None,
            logger,
        }
    }

    pub async fn run(
        &mut self,
        hub_url: String,
        supervisor: Arc<CoreSupervisor>,
    ) -> anyhow::Result<()> {
        let mut retry_delay = Duration::from_secs(1);
        let max_retry_delay = Duration::from_secs(60);

        loop {
            match self.try_connect(&hub_url, supervisor.clone()).await {
                Ok(()) => {
                    tracing::info!("agent stream ended, reconnecting...");
                    retry_delay = Duration::from_secs(1);
                }
                Err(e) => {
                    tracing::warn!("connection error: {}, retry in {:?}", e, retry_delay);
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = std::cmp::min(retry_delay * 2, max_retry_delay);
                }
            }
        }
    }

    async fn try_connect(
        &mut self,
        hub_url: &str,
        supervisor: Arc<CoreSupervisor>,
    ) -> anyhow::Result<()> {
        let endpoint = tonic::transport::Endpoint::new(hub_url.to_string())?;
        let endpoint = if let Some(tls) = &self.tls_config {
            endpoint.tls_config(tls.clone())?
        } else {
            endpoint
        };
        let mut client = HubAgentClient::<Channel>::connect(endpoint).await?;

        // Channel for outbound messages (Agent -> Hub)
        let (outbound_tx, outbound_rx) = mpsc::channel::<AgentMessage>(128);
        let outbound_stream = ReceiverStream::new(outbound_rx);

        // Start bidirectional stream
        let mut stream = client.stream(outbound_stream).await?.into_inner();

        // Report per-core applied config versions so the Hub can skip pushes
        // for configs this agent already runs (avoids redundant restarts).
        let core_config_versions: std::collections::HashMap<String, String> =
            persist::load_last_configs(&self.data_dir)
                .await
                .into_iter()
                .filter(|(_, snapshot)| !snapshot.version.is_empty())
                .map(|(core_type, snapshot)| (core_type.to_string(), snapshot.version))
                .collect();

        // Send register request
        let register_msg = AgentMessage {
            payload: Some(pp_proto::agent_message::Payload::Register(
                RegisterRequest {
                    agent_id: self.agent_id.clone(),
                    token: self.token.clone(),
                    hostname: self.hostname.clone(),
                    domain: self.domain.clone(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    capabilities: vec!["sing-box".to_string(), "mihomo".to_string()],
                    labels: Default::default(),
                    core_config_versions,
                },
            )),
        };
        outbound_tx.send(register_msg).await?;

        // Clone for heartbeat task
        let heartbeat_tx = outbound_tx.clone();
        let agent_id = self.agent_id.clone();

        // Heartbeat + metrics task
        let heartbeat_handle = tokio::spawn(async move {
            let mut heartbeat_ticker = tokio::time::interval(Duration::from_secs(30));
            let mut metrics_ticker = tokio::time::interval(Duration::from_secs(60));
            let mut online_users_ticker = tokio::time::interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = heartbeat_ticker.tick() => {
                        let msg = AgentMessage {
                            payload: Some(pp_proto::agent_message::Payload::Heartbeat(
                                Heartbeat {
                                    timestamp: chrono::Utc::now().timestamp(),
                                    status: pp_proto::NodeStatus::Online as i32,
                                },
                            )),
                        };
                        if heartbeat_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    _ = metrics_ticker.tick() => {
                        if let Some(metrics) = collect_host_metrics() {
                            let msg = AgentMessage {
                                payload: Some(pp_proto::agent_message::Payload::Metrics(metrics)),
                            };
                            if heartbeat_tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    _ = online_users_ticker.tick() => {
                        let users = collect_online_users().await;
                        let msg = AgentMessage {
                            payload: Some(pp_proto::agent_message::Payload::OnlineUsers(
                                OnlineUsersReport {
                                    timestamp: chrono::Utc::now().timestamp(),
                                    users,
                                }
                            )),
                        };
                        if heartbeat_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
            }
            tracing::debug!("heartbeat task for agent {} stopped", agent_id);
        });

        // Log sender task
        let log_tx = outbound_tx.clone();
        let log_rx = self.logger.receiver();
        let log_agent_id = self.agent_id.clone();
        let log_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            let mut stopped = false;
            while !stopped {
                interval.tick().await;
                let batch = collect_logs(&mut *log_rx.lock().await, 100).await;
                if !batch.entries.is_empty() {
                    let msg = AgentMessage {
                        payload: Some(pp_proto::agent_message::Payload::Logs(batch)),
                    };
                    if log_tx.send(msg).await.is_err() {
                        stopped = true;
                    }
                }
            }
            tracing::debug!("log sender task for agent {} stopped", log_agent_id);
        });

        // Core status reporter task
        let status_tx = outbound_tx.clone();
        let status_supervisor = supervisor.clone();
        let status_agent_id = self.agent_id.clone();
        let status_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            let mut stopped = false;
            while !stopped {
                interval.tick().await;
                let reports = collect_core_status(&status_supervisor).await;
                for report in reports {
                    let msg = AgentMessage {
                        payload: Some(pp_proto::agent_message::Payload::CoreStatus(report)),
                    };
                    if status_tx.send(msg).await.is_err() {
                        stopped = true;
                        break;
                    }
                }
            }
            tracing::debug!("core status task for agent {} stopped", status_agent_id);
        });

        // Traffic reporter task (interval configurable via
        // PROXYPANEL_TRAFFIC_REPORT_INTERVAL_SECS, default 60s)
        let traffic_tx = outbound_tx.clone();
        let traffic_handle =
            crate::reporter::spawn_traffic_reporter(traffic_tx, traffic_report_interval_secs());

        // Certificate renewal task (first pass runs immediately on connect)
        let renew_tx = outbound_tx.clone();
        let renew_supervisor = supervisor.clone();
        let renew_data_dir = self.data_dir.clone();
        let renew_handle = tokio::spawn(async move {
            crate::acme::run_renewal_loop(renew_data_dir, renew_supervisor, renew_tx).await;
        });

        // Inbound message loop (Hub -> Agent)
        let result: anyhow::Result<()> = async {
            while let Some(msg_result) = stream.message().await? {
                if let Err(e) =
                    handle_hub_message(msg_result, &supervisor, outbound_tx.clone(), &self.data_dir)
                        .await
                {
                    tracing::warn!("error handling hub message: {}", e);
                }
            }
            Ok(())
        }
        .await;

        heartbeat_handle.abort();
        log_handle.abort();
        status_handle.abort();
        traffic_handle.abort();
        renew_handle.abort();
        result
    }
}

async fn handle_hub_message(
    msg: HubMessage,
    supervisor: &CoreSupervisor,
    outbound: mpsc::Sender<AgentMessage>,
    data_dir: &std::path::Path,
) -> anyhow::Result<()> {
    use pp_proto::hub_message::Payload;

    match msg.payload {
        Some(Payload::RegisterResp(resp)) => {
            tracing::info!(
                "register response: success={}, msg={}",
                resp.success,
                resp.message
            );
        }
        Some(Payload::ConfigPush(push)) => {
            tracing::info!(
                "received config push, core={:?}, restart={}",
                push.target_core,
                push.restart_required
            );
            handle_config_push(supervisor, push, data_dir).await?;
        }
        Some(Payload::ConfigReload(reload)) => {
            tracing::info!("received config reload, core={:?}", reload.target_core);
            let push = ConfigPush {
                config_json: reload.config_json,
                target_core: reload.target_core,
                restart_required: false,
                config_version: reload.config_version,
                core_version: reload.core_version,
                core_build_id: String::new(),
            };
            handle_config_push(supervisor, push, data_dir).await?;
        }
        Some(Payload::CoreCmd(cmd)) => {
            handle_core_command(supervisor, cmd).await?;
        }
        Some(Payload::CoreBinaryList(_)) => {
            let binaries = list_core_binaries(supervisor).await;
            outbound
                .send(AgentMessage {
                    payload: Some(pp_proto::agent_message::Payload::CoreBinaries(
                        pp_proto::CoreBinaryList {
                            binaries,
                            error: String::new(),
                        },
                    )),
                })
                .await?;
        }
        Some(Payload::CoreBinaryDelete(del)) => {
            let (binaries, error) = match delete_core_binary(supervisor, &del.file_name).await {
                Ok(()) => (list_core_binaries(supervisor).await, String::new()),
                Err(e) => (list_core_binaries(supervisor).await, e),
            };
            outbound
                .send(AgentMessage {
                    payload: Some(pp_proto::agent_message::Payload::CoreBinaries(
                        pp_proto::CoreBinaryList { binaries, error },
                    )),
                })
                .await?;
        }
        Some(Payload::CertIssue(issue)) => {
            tracing::info!("received cert issue request for {}", issue.domain);
            let data_dir = data_dir.to_path_buf();
            let report = outbound.clone();
            tokio::spawn(async move {
                match crate::acme::issue_certificate(&data_dir, &issue.cert_id, &issue.domain).await
                {
                    Ok(expires_at) => {
                        crate::acme::send_cert_status(
                            &report,
                            &issue.cert_id,
                            "active",
                            expires_at,
                            "",
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!("cert issuance for {} failed: {}", issue.domain, e);
                        crate::acme::send_cert_status(
                            &report,
                            &issue.cert_id,
                            "failed",
                            0,
                            &e.to_string(),
                        )
                        .await;
                    }
                }
            });
        }
        Some(Payload::Shutdown(shutdown)) => {
            tracing::info!(
                "received shutdown command: {}, delay={}s",
                shutdown.reason,
                shutdown.delay_sec
            );
            tokio::time::sleep(Duration::from_secs(shutdown.delay_sec as u64)).await;
            supervisor.stop_all().await?;
            std::process::exit(0);
        }
        None => {}
    }

    Ok(())
}

async fn handle_config_push(
    supervisor: &CoreSupervisor,
    push: ConfigPush,
    data_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let core_type = core_type_from_i32(push.target_core);
    let config: serde_json::Value = serde_json::from_str(&push.config_json)?;

    if !push.restart_required && !push.config_version.is_empty() {
        let applied_version = persist::load_last_configs(data_dir)
            .await
            .into_iter()
            .find(|(core, _)| *core == core_type)
            .map(|(_, snapshot)| snapshot.version);
        if applied_version.as_deref() == Some(push.config_version.as_str()) {
            tracing::info!(
                "config version {} already applied for {:?}, skipping",
                push.config_version,
                core_type
            );
            return Ok(());
        }
    }

    let manager = if let Some(manager) = supervisor.get(core_type).await {
        manager
    } else {
        tracing::info!("core {:?} not registered; installing on demand", core_type);
        let version = if push.core_version.is_empty() {
            None
        } else {
            Some(push.core_version.as_str())
        };
        supervisor
            .ensure_manager_from_discovered(core_type, version)
            .await
            .map_err(|e| anyhow::anyhow!("failed to install {:?}: {}", core_type, e))?
    };

    // Rolling-tag build upgrade: when the Hub reports a newer upstream build
    // for the pinned version, or a pinned version change, re-download
    // the binary before applying config.
    if !push.core_build_id.is_empty() || !push.core_version.is_empty() {
        if let Err(e) = ensure_core_build(
            supervisor,
            core_type,
            &push.core_version,
            &push.core_build_id,
        )
        .await
        {
            tracing::warn!("core build upgrade failed for {:?}: {}", core_type, e);
        }
    }

    if push.restart_required {
        manager
            .restart(&config)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    } else {
        manager
            .reload(&config)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    tracing::info!("config applied for {:?}", core_type);

    if let Err(e) =
        persist::save_last_config(data_dir, core_type, &config, &push.config_version).await
    {
        tracing::warn!("failed to persist config snapshot: {}", e);
    }

    Ok(())
}

/// Re-download a core binary when the upstream build of its pinned version
/// changed (rolling tags keep the same version string across builds).
///
/// The previous build marker lives in `<bin_dir>/.build_id.<core>`; when it
/// differs from the Hub-reported build, the binary is removed and fetched
/// again. Deleting the binary of a running core is safe: the process keeps
/// its inode until the restart that follows the config push.
async fn ensure_core_build(
    supervisor: &CoreSupervisor,
    core_type: pp_common::CoreType,
    version: &str,
    build_id: &str,
) -> anyhow::Result<()> {
    let Some(bin_dir) = supervisor.bin_dir().await else {
        return Ok(());
    };
    let marker = bin_dir.join(format!(".build_id.{}", core_type));
    let expected = format!("{}|{}", version, build_id);
    let current = tokio::fs::read_to_string(&marker).await.unwrap_or_default();
    if current.trim() == expected {
        return Ok(());
    }

    tracing::info!(
        "upgrading {:?} binary to upstream build {}",
        core_type,
        build_id
    );
    let binary = pp_core::core_binary_path(&bin_dir, core_type);
    // Move the current binary aside first: if the download fails the core
    // must be able to restart with the old build.
    let backup = bin_dir.join(format!(".backup.{}", core_type));
    let has_binary = tokio::fs::try_exists(&binary).await.unwrap_or(false);
    if has_binary {
        tokio::fs::rename(&binary, &backup).await?;
    }
    let version = if version.is_empty() {
        None
    } else {
        Some(version)
    };
    match pp_core::ensure_core_binary(&bin_dir, core_type, version).await {
        Ok(_) => {
            let _ = tokio::fs::remove_file(&backup).await;
            tokio::fs::write(&marker, &expected).await?;
            Ok(())
        }
        Err(e) => {
            if has_binary {
                let _ = tokio::fs::rename(&backup, &binary).await;
            }
            Err(anyhow::anyhow!("{}", e))
        }
    }
}

/// List the core binaries present in the agent's bin directory.
async fn list_core_binaries(supervisor: &CoreSupervisor) -> Vec<pp_proto::CoreBinary> {
    let Some(bin_dir) = supervisor.bin_dir().await else {
        return Vec::new();
    };

    let mut in_use = std::collections::HashSet::new();
    for core in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if supervisor.get(core).await.is_some() {
            in_use.insert(pp_core::core_binary_path(&bin_dir, core));
        }
    }

    let mut binaries = Vec::new();
    let mut entries = match tokio::fs::read_dir(&bin_dir).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("failed to read bin dir {}: {}", bin_dir.display(), e);
            return binaries;
        }
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        binaries.push(pp_proto::CoreBinary {
            file_name: name,
            size_bytes: meta.len() as i64,
            modified_at,
            in_use: in_use.contains(&entry.path()),
        });
    }
    binaries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    binaries
}

/// Delete a core binary from the bin directory, refusing anything that is
/// not a plain file name or is currently used by a registered core.
async fn delete_core_binary(supervisor: &CoreSupervisor, file_name: &str) -> Result<(), String> {
    if file_name.is_empty()
        || file_name.starts_with('.')
        || file_name.contains('/')
        || file_name.contains('\\')
    {
        return Err("invalid file name".to_string());
    }
    let Some(bin_dir) = supervisor.bin_dir().await else {
        return Err("bin dir unknown".to_string());
    };
    let path = bin_dir.join(file_name);

    for core in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if supervisor.get(core).await.is_some() && pp_core::core_binary_path(&bin_dir, core) == path
        {
            return Err(format!("{} is in use by a running core", file_name));
        }
    }

    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| format!("failed to delete {}: {}", file_name, e))
}

async fn handle_core_command(supervisor: &CoreSupervisor, cmd: CoreCommand) -> anyhow::Result<()> {
    use pp_proto::core_command::Command;

    match cmd.command {
        Some(Command::Start(start)) => {
            let core_type = core_type_from_i32(start.core_type);
            tracing::info!("received start command for {:?}", core_type);

            let manager = if let Some(mgr) = supervisor.get(core_type).await {
                mgr
            } else {
                supervisor
                    .ensure_manager_from_discovered(core_type, None)
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to prepare {:?}: {}", core_type, e))?
            };

            if manager.is_running().await {
                tracing::info!("{:?} is already running", core_type);
                return Ok(());
            }

            let empty_config = minimal_config_for_core(core_type);
            manager
                .start(&empty_config)
                .await
                .map_err(|e| anyhow::anyhow!("failed to start {:?}: {}", core_type, e))?;

            tracing::info!("{:?} started successfully", core_type);
        }
        Some(Command::Stop(stop)) => {
            let core_type = core_type_from_i32(stop.core_type);
            if let Some(manager) = supervisor.get(core_type).await {
                manager.stop().await.map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
        Some(Command::Restart(restart)) => {
            let core_type = core_type_from_i32(restart.core_type);
            let manager = if let Some(mgr) = supervisor.get(core_type).await {
                mgr
            } else {
                supervisor
                    .ensure_manager_from_discovered(core_type, None)
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to prepare {:?}: {}", core_type, e))?
            };

            let empty_config = minimal_config_for_core(core_type);
            manager
                .restart(&empty_config)
                .await
                .map_err(|e| anyhow::anyhow!("failed to restart {:?}: {}", core_type, e))?;
        }
        None => {}
    }

    Ok(())
}

/// Return a minimal valid config that allows the core process to start.
/// The real config will be provided by Hub via ConfigPush.
fn minimal_config_for_core(core_type: pp_common::CoreType) -> serde_json::Value {
    match core_type {
        pp_common::CoreType::SingBox => serde_json::json!({
            "log": { "level": "warn" },
            "inbounds": [],
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        }),
        pp_common::CoreType::Mihomo => serde_json::json!({
            "log-level": "warning",
            "mode": "rule",
            "allow-lan": false,
            "listeners": [],
            "rules": ["MATCH,DIRECT"]
        }),
    }
}

fn collect_host_metrics() -> Option<HostMetrics> {
    use sysinfo::{MemoryRefreshKind, Networks, RefreshKind, System};

    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(sysinfo::CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    std::thread::sleep(std::time::Duration::from_millis(500));
    sys.refresh_specifics(RefreshKind::nothing().with_cpu(sysinfo::CpuRefreshKind::everything()));

    let cpu_percent = sys.global_cpu_usage();
    let mem_used = sys.used_memory();
    let mem_total = sys.total_memory();

    // Collect disk usage
    let (disk_used, disk_total) = collect_disk_usage();

    // Collect network interfaces
    let networks = Networks::new_with_refreshed_list();
    let net = networks
        .iter()
        .map(|(name, data)| pp_proto::NetInterface {
            name: name.clone(),
            rx_bytes: data.total_received(),
            tx_bytes: data.total_transmitted(),
            rx_packets: data.total_packets_received(),
            tx_packets: data.total_packets_transmitted(),
        })
        .collect();

    // Collect load averages (Unix only)
    let (load_avg_1, load_avg_5, load_avg_15) = collect_load_average();

    Some(HostMetrics {
        timestamp: chrono::Utc::now().timestamp(),
        cpu_percent,
        mem_used,
        mem_total,
        disk_used,
        disk_total,
        net,
        load_avg_1,
        load_avg_5,
        load_avg_15,
    })
}

/// Collect disk usage for the root filesystem.
fn collect_disk_usage() -> (u64, u64) {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    match disks.iter().next() {
        Some(disk) => (
            disk.total_space() - disk.available_space(),
            disk.total_space(),
        ),
        None => (0, 0),
    }
}

/// Collect system load averages.
fn collect_load_average() -> (f32, f32, f32) {
    #[cfg(unix)]
    {
        let avg = sysinfo::System::load_average();
        (avg.one as f32, avg.five as f32, avg.fifteen as f32)
    }
    #[cfg(not(unix))]
    {
        (0.0, 0.0, 0.0)
    }
}

/// Collect currently online users from running proxy cores.
/// Returns an empty list if core APIs are unavailable.
async fn collect_online_users() -> Vec<OnlineUser> {
    match pp_core::query_all_online_users().await {
        Ok(users) => users
            .into_iter()
            .map(|u| OnlineUser {
                client_id: u.client_id,
                email: u.email,
                ip_address: u.ip_address,
                inbound_tag: u.inbound_tag.unwrap_or_default(),
            })
            .collect(),
        Err(e) => {
            tracing::warn!("failed to collect online users: {}", e);
            vec![]
        }
    }
}

fn core_type_from_i32(value: i32) -> pp_common::CoreType {
    match value {
        2 => pp_common::CoreType::SingBox,
        4 => pp_common::CoreType::Mihomo,
        _ => pp_common::CoreType::SingBox,
    }
}

/// Traffic report interval in seconds (`PROXYPANEL_TRAFFIC_REPORT_INTERVAL_SECS`,
/// default 60, clamped to at least 10).
fn traffic_report_interval_secs() -> u64 {
    std::env::var("PROXYPANEL_TRAFFIC_REPORT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .max(10)
}
