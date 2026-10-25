use super::*;
use crate::config::HubConfig;
use crate::rate_limiter::RateLimiter;
use execution::update_arguments;
use status::MaintenanceOutcome;

fn release(tag: &str) -> Release {
    Release {
        tag: tag.to_string(),
        version: parse_version(tag).unwrap(),
        html_url: format!("https://github.com/owner/repo/releases/tag/{tag}"),
        published_at: None,
    }
}

fn test_state(config: HubConfig) -> Arc<AppState> {
    let db = sea_orm::DatabaseConnection::default();
    AppState::new(db, config, RateLimiter::default(), None)
}

#[test]
fn system_scopes_are_registered() {
    use crate::middleware::api_key::scopes;

    assert!(scopes::ALL_SCOPES.contains(&scopes::SYSTEM_READ));
    assert!(scopes::ALL_SCOPES.contains(&scopes::SYSTEM_WRITE));
    assert_eq!(scopes::SYSTEM_READ, "system:read");
    assert_eq!(scopes::SYSTEM_WRITE, "system:write");
}

#[test]
fn command_arguments_are_fixed_and_version_is_validated() {
    let config = HubConfig::default();
    let release = release("v99.1.2");
    let arguments = update_arguments(&config, &release);
    let expected = vec![
        "-n",
        "/usr/bin/systemd-run",
        "--unit=proxy-panel-hub-update",
        "--collect",
        "/usr/local/bin/proxy-panel",
        "upgrade",
        "hub",
        "--version",
        "v99.1.2",
        "--repo",
        "ybakiame/multi-proxy-panel",
        "--status-file",
        "data/hub-update.json",
    ];
    assert_eq!(arguments, expected);

    assert!(parse_version("v1.2.3;touch /tmp/pwned").is_err());
    assert!(validate_target(&release).is_ok());
}

#[test]
fn equal_older_and_prerelease_targets_are_rejected() {
    assert_eq!(
        validate_target(&release(CURRENT_VERSION)).unwrap_err().code,
        "no_update_available"
    );
    assert_eq!(
        validate_target(&release("v0.0.1")).unwrap_err().code,
        "no_update_available"
    );
    assert_eq!(
        validate_target(&release("v99.0.0-rc.1")).unwrap_err().code,
        "invalid_target_version"
    );
}

#[tokio::test]
async fn disabled_update_is_forbidden_before_release_lookup() {
    let state = test_state(HubConfig::default());
    let error = start_update(State(state), None).await.unwrap_err();
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.code, "maintenance_disabled");
}

#[tokio::test]
async fn running_file_and_in_process_task_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let config = HubConfig {
        maintenance_enabled: true,
        maintenance_status_file: temp.path().join("status.json"),
        ..Default::default()
    };
    let state = test_state(config);
    let running = MaintenanceStatus::running(CURRENT_VERSION, "99.0.0");
    write_status(&state.config.maintenance_status_file, &running)
        .await
        .unwrap();
    let error = start_update(State(state.clone()), None).await.unwrap_err();
    assert_eq!(error.status, StatusCode::CONFLICT);
    assert_eq!(error.code, "update_in_progress");

    write_status(
        &state.config.maintenance_status_file,
        &running.failed("stopped".to_string()),
    )
    .await
    .unwrap();
    assert!(state.maintenance.try_begin_update());
    let error = start_update(State(state), None).await.unwrap_err();
    assert_eq!(error.code, "update_in_progress");
}

#[tokio::test]
async fn missing_corrupt_stale_and_recovered_statuses_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("status.json");
    let idle = read_status(&path, CURRENT_VERSION).await.unwrap();
    assert_eq!(idle.status, MaintenanceOutcome::Idle);
    assert_eq!(idle.current_version.as_deref(), Some(CURRENT_VERSION));

    tokio::fs::write(&path, b"not json").await.unwrap();
    let error = read_status(&path, CURRENT_VERSION).await.unwrap_err();
    assert_eq!(error.code, "invalid_status_file");

    let mut stale = MaintenanceStatus::running(CURRENT_VERSION, "99.0.0");
    stale.started_at = Some(Utc::now() - chrono::Duration::minutes(31));
    write_status(&path, &stale).await.unwrap();
    let stale = read_status(&path, CURRENT_VERSION).await.unwrap();
    assert_eq!(stale.status, MaintenanceOutcome::Stale);
    assert!(stale.message.contains("manual verification"));

    let recovered = stale.failed("manually recovered".to_string());
    write_status(&path, &recovered).await.unwrap();
    assert_eq!(
        read_status(&path, CURRENT_VERSION).await.unwrap().status,
        MaintenanceOutcome::Failed
    );
}

#[tokio::test]
async fn spawn_failure_is_recorded_and_releases_process_lock() {
    let temp = tempfile::tempdir().unwrap();
    let config = HubConfig {
        maintenance_enabled: true,
        maintenance_status_file: temp.path().join("status.json"),
        maintenance_sudo_path: temp.path().join("missing-sudo"),
        ..Default::default()
    };
    let state = test_state(config);
    let release = release("v99.0.0");
    assert!(state.maintenance.try_begin_update());
    let error = execute_update(state.clone(), &release).await.unwrap_err();
    state.maintenance.finish_update();
    assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
    let status = read_status(&state.config.maintenance_status_file, CURRENT_VERSION)
        .await
        .unwrap();
    assert_eq!(status.status, MaintenanceOutcome::Failed);
    assert!(status.finished_at.is_some());
    assert!(!state.maintenance.update_in_progress());
}

#[cfg(unix)]
#[tokio::test]
async fn accepted_update_spawns_the_fixed_argument_command() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let sudo = temp.path().join("fake-sudo");
    let captured = temp.path().join("arguments");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
        captured.display()
    );
    tokio::fs::write(&sudo, script).await.unwrap();
    let mut permissions = std::fs::metadata(&sudo).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&sudo, permissions).unwrap();

    let config = HubConfig {
        maintenance_enabled: true,
        maintenance_status_file: temp.path().join("status.json"),
        maintenance_cli_path: "/test/proxy-panel".into(),
        maintenance_sudo_path: sudo,
        maintenance_systemd_run_path: "/test/systemd-run".into(),
        release_repo: "owner/repo".to_string(),
        ..Default::default()
    };
    let state = test_state(config);
    assert!(state.maintenance.try_begin_update());
    let response = execute_update(state.clone(), &release("v99.0.0"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while state.maintenance.update_in_progress() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let arguments = tokio::fs::read_to_string(captured).await.unwrap();
    assert_eq!(
        arguments,
        "-n\n/test/systemd-run\n--unit=proxy-panel-hub-update\n--collect\n/test/proxy-panel\nupgrade\nhub\n--version\nv99.0.0\n--repo\nowner/repo\n--status-file\n"
            .to_string()
                + state.config.maintenance_status_file.to_str().unwrap()
                + "\n"
    );
    assert_eq!(
        read_status(&state.config.maintenance_status_file, CURRENT_VERSION)
            .await
            .unwrap()
            .status,
        MaintenanceOutcome::Running
    );
}

#[cfg(unix)]
#[tokio::test]
async fn nonzero_exit_preserves_terminal_status_written_by_cli() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let sudo = temp.path().join("fake-sudo");
    let status_file = temp.path().join("status.json");
    let terminal = MaintenanceStatus {
        status: MaintenanceOutcome::RolledBack,
        stage: "restarting".to_string(),
        component: "hub".to_string(),
        current_version: Some(CURRENT_VERSION.to_string()),
        target_version: Some("99.0.0".to_string()),
        message: "rolled back by CLI".to_string(),
        started_at: Some(Utc::now()),
        finished_at: Some(Utc::now()),
    };
    let terminal_json = serde_json::to_string(&terminal).unwrap();
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' '{terminal_json}' > '{}'\nexit 1\n",
        status_file.display()
    );
    tokio::fs::write(&sudo, script).await.unwrap();
    let mut permissions = std::fs::metadata(&sudo).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&sudo, permissions).unwrap();

    let config = HubConfig {
        maintenance_enabled: true,
        maintenance_status_file: status_file,
        maintenance_sudo_path: sudo,
        ..Default::default()
    };
    let state = test_state(config);
    assert!(state.maintenance.try_begin_update());
    let response = execute_update(state.clone(), &release("v99.0.0"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while state.maintenance.update_in_progress() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let status = read_status(&state.config.maintenance_status_file, CURRENT_VERSION)
        .await
        .unwrap();
    assert_eq!(status.status, MaintenanceOutcome::RolledBack);
    assert_eq!(status.message, "rolled back by CLI");
}
