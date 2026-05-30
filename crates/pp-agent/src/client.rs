use pp_proto::{
    AgentMessage, ConfigPush, CoreCommand, Heartbeat, HostMetrics, HubMessage, OnlineUser,
    OnlineUsersReport, RegisterRequest, hub_agent_client::HubAgentClient,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;

use pp_core::CoreSupervisor;

pub struct AgentStreamClient {
    agent_id: String,
    token: String,
    hostname: String,
    #[allow(dead_code)]
    hub_tx: mpsc::Sender<AgentMessage>,
    #[allow(dead_code)]
    hub_rx: Option<mpsc::Receiver<HubMessage>>,
}

impl AgentStreamClient {
    pub fn new(token: String, hostname: String) -> Self {
        let agent_id = pp_common::generate_uuid();
        let (hub_tx, _hub_rx) = mpsc::channel::<AgentMessage>(128);
        Self {
            agent_id,
            token,
            hostname,
            hub_tx,
            hub_rx: None,
        }
    }

    pub async fn run(&mut self, hub_url: String, supervisor: CoreSupervisor) -> anyhow::Result<()> {
        let mut retry_delay = Duration::from_secs(1);
        let max_retry_delay = Duration::from_secs(60);

        loop {
            match self.try_connect(&hub_url, &supervisor).await {
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
        supervisor: &CoreSupervisor,
    ) -> anyhow::Result<()> {
        let mut client = HubAgentClient::<Channel>::connect(hub_url.to_string()).await?;

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
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    capabilities: vec!["xray".to_string(), "sing-box".to_string()],
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

        // Inbound message loop (Hub -> Agent)
        let result: anyhow::Result<()> = async {
            while let Some(msg_result) = stream.message().await? {
                if let Err(e) =
                    handle_hub_message(msg_result, supervisor, outbound_tx.clone()).await
                {
                    tracing::warn!("error handling hub message: {}", e);
                }
            }
            Ok(())
        }
        .await;

        heartbeat_handle.abort();
        result
    }
}

async fn handle_hub_message(
    msg: HubMessage,
    supervisor: &CoreSupervisor,
    _outbound: mpsc::Sender<AgentMessage>,
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
            handle_config_push(supervisor, push).await?;
        }
        Some(Payload::ConfigReload(reload)) => {
            tracing::info!("received config reload, core={:?}", reload.target_core);
            let push = ConfigPush {
                config_json: reload.config_json,
                target_core: reload.target_core,
                restart_required: false,
                config_version: reload.config_version,
            };
            handle_config_push(supervisor, push).await?;
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

async fn handle_config_push(supervisor: &CoreSupervisor, push: ConfigPush) -> anyhow::Result<()> {
    let core_type = core_type_from_i32(push.target_core);
    let config: serde_json::Value = serde_json::from_str(&push.config_json)?;

    if let Some(manager) = supervisor.get(core_type).await {
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
    } else {
        tracing::warn!("no manager found for core type {:?}", core_type);
    }

    Ok(())
}

async fn handle_core_command(supervisor: &CoreSupervisor, cmd: CoreCommand) -> anyhow::Result<()> {
    use pp_proto::core_command::Command;

    match cmd.command {
        Some(Command::Start(start)) => {
            let core_type = core_type_from_i32(start.core_type);
            tracing::info!("received start command for {:?}", core_type);
            // Core would need a blank config or cached config to start
        }
        Some(Command::Stop(stop)) => {
            let core_type = core_type_from_i32(stop.core_type);
            if let Some(manager) = supervisor.get(core_type).await {
                manager.stop().await.map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
        Some(Command::Restart(restart)) => {
            let core_type = core_type_from_i32(restart.core_type);
            if let Some(manager) = supervisor.get(core_type).await {
                // Need cached config; for now just restart with empty config
                let empty = serde_json::json!({});
                manager
                    .restart(&empty)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
        None => {}
    }

    Ok(())
}

fn collect_host_metrics() -> Option<HostMetrics> {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};

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

    Some(HostMetrics {
        timestamp: chrono::Utc::now().timestamp(),
        cpu_percent,
        mem_used,
        mem_total,
        disk_used: 0, // TODO: implement disk usage
        disk_total: 0,
        net: vec![],
        load_avg_1: 0.0,
        load_avg_5: 0.0,
        load_avg_15: 0.0,
    })
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
        3 => pp_common::CoreType::Both,
        _ => pp_common::CoreType::SingBox,
    }
}
