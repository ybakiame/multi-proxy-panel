use std::ffi::OsString;

use pp_common::release::Release;
use tokio::process::{Child, Command};

use crate::config::HubConfig;

pub(super) fn update_arguments(config: &HubConfig, release: &Release) -> Vec<OsString> {
    vec![
        OsString::from("-n"),
        config.maintenance_systemd_run_path.as_os_str().to_owned(),
        OsString::from("--unit=proxy-panel-hub-update"),
        OsString::from("--collect"),
        config.maintenance_cli_path.as_os_str().to_owned(),
        OsString::from("upgrade"),
        OsString::from("hub"),
        OsString::from("--version"),
        OsString::from(&release.tag),
        OsString::from("--repo"),
        OsString::from(&config.release_repo),
        OsString::from("--status-file"),
        config.maintenance_status_file.as_os_str().to_owned(),
    ]
}

pub(super) fn spawn_update(config: &HubConfig, release: &Release) -> std::io::Result<Child> {
    Command::new(&config.maintenance_sudo_path)
        .args(update_arguments(config, release))
        .spawn()
}
