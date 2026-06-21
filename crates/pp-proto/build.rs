use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let proto_dir = PathBuf::from(manifest_dir)
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("proto");

    let hub_proto = proto_dir.join("hub_agent.proto");
    let xray_stats_proto = proto_dir.join("xray_stats.proto");
    let singbox_daemon_proto = proto_dir.join("singbox_daemon.proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &[hub_proto, xray_stats_proto, singbox_daemon_proto],
            &[proto_dir],
        )?;

    Ok(())
}
