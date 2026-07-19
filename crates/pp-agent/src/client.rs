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

        // Send register request
        let register_msg = AgentMessage {
            payload: Some(pp_proto::agent_message::Payload::Register(
                RegisterRequest {
                    agent_id: self.agent_id.clone(),
                    token: self.token.clone(),
                    hostname: self.hostname.clone(),
                    domain: self.domain.clone(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    capabilities: vec![
                        "xray".to_string(),
                        "sing-box".to_string(),
                        "mihomo".to_string(),
                    ],
                    labels: Default::default(),
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

        // Traffic reporter task
        let traffic_tx = outbound_tx.clone();
        let traffic_handle = crate::reporter::spawn_traffic_reporter(traffic_tx, 60);

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
        result
    }
}

async fn handle_hub_message(
    msg: HubMessage,
    supervisor: &CoreSupervisor,
    _outbound: mpsc::Sender<AgentMessage>,
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
            };
            handle_config_push(supervisor, push, data_dir).await?;
        }
        Some(Payload::CoreCmd(cmd)) => {
            handle_core_command(supervisor, cmd).await?;
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

    if let Err(e) = persist::save_last_config(data_dir, core_type, &config).await {
        tracing::warn!("failed to persist config snapshot: {}", e);
    }

    Ok(())
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
        pp_common::CoreType::Xray => serde_json::json!({
            "log": { "loglevel": "warning" },
            "inbounds": [],
            "outbounds": [{ "protocol": "freedom", "tag": "direct" }]
        }),
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
        1 => pp_common::CoreType::Xray,
        2 => pp_common::CoreType::SingBox,
        4 => pp_common::CoreType::Mihomo,
        _ => pp_common::CoreType::SingBox,
    }
}
