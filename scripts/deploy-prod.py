#!/usr/bin/env python3
"""Production deployment and update script for ProxyPanel.

Supports inventory-based batch deployment of Hub + Agent or Agent-only nodes.
Designed for safer production use than deploy-server.py:

  - SSH key authentication by default (password only as fallback)
  - Strict host-key verification (configurable)
  - Secrets via environment variable substitution, never committed
  - Backup before update with automatic rollback on failure
  - Inventory file (YAML or JSON) for multi-server batches
  - Separate `deploy` and `update` actions

Example:
    # Generate a token first (optional, recommended for production)
    export TEST2_AGENT_TOKEN="$(./target/release/proxy-panel gen-token)"
    export TEST2_AGENT_ID="$(./target/release/proxy-panel provision-node \\
      --database-url 'sqlite:///opt/proxy-panel/data/proxypanel.db?mode=rwc' \\
      --name test2 --address 192.3.150.233 --token "$TEST2_AGENT_TOKEN" \\
      | awk -F': ' '/^node_id:/ {print $2}')"

    python3 scripts/deploy-prod.py --inventory deploy-prod.yaml --action deploy
    python3 scripts/deploy-prod.py --inventory deploy-prod.yaml --action update

    # Save generated secrets to a file (optional, for first deploy)
    python3 scripts/deploy-prod.py --inventory deploy-prod.yaml --action deploy --secrets-out .deploy-secrets.env

See scripts/deploy-prod.example.yaml for the inventory format.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Tuple

CHUNK_SIZE = 1024 * 1024  # 1 MiB
MAX_RETRIES = 5

REMOTE_DIR = "/opt/proxy-panel"
REMOTE_BIN = "/usr/local/bin"
REMOTE_ETC = "/etc/proxy-panel"
REMOTE_WEB = REMOTE_DIR + "/web/dist"
REMOTE_DATA = REMOTE_DIR + "/data"
REMOTE_AGENT_BIN = REMOTE_DIR + "/bin"
REMOTE_BACKUP_DIR = REMOTE_DIR + "/backups"

LOCAL_ROOT = Path(__file__).resolve().parent.parent


# ---------------------------------------------------------------------------
# Configuration loading
# ---------------------------------------------------------------------------

def load_inventory(path: str) -> dict:
    path = Path(path)
    if not path.exists():
        raise FileNotFoundError("Inventory file not found: %s" % path)
    text = path.read_text(encoding="utf-8")
    if path.suffix in (".yaml", ".yml"):
        try:
            import yaml
            return yaml.safe_load(text) or {}
        except ImportError as e:
            raise RuntimeError(
                "PyYAML is required for YAML inventories. Install it with: "
                "pip install pyyaml (or use a JSON inventory instead)."
            ) from e
    return json.loads(text)


def substitute_env_vars(value):
    """Recursively replace ${VAR} or ${VAR:-default} with environment values."""
    if isinstance(value, dict):
        return {k: substitute_env_vars(v) for k, v in value.items()}
    if isinstance(value, list):
        return [substitute_env_vars(v) for v in value]
    if isinstance(value, str):
        pattern = re.compile(r"\$\{([^}:]+)(?::-(.*))?\}")

        def repl(match):
            var_name = match.group(1)
            default = match.group(2)
            env_value = os.environ.get(var_name)
            if env_value is None:
                if default is not None:
                    return default
                raise RuntimeError(
                    "Environment variable %s is required but not set" % var_name
                )
            return env_value

        return pattern.sub(repl, value)
    return value


# ---------------------------------------------------------------------------
# SSH helpers
# ---------------------------------------------------------------------------

def resolve_ssh_key(path: Optional[str]) -> Optional[str]:
    if not path:
        return None
    p = Path(path).expanduser()
    if not p.exists():
        raise FileNotFoundError("SSH key not found: %s" % p)
    return str(p)


def connect_ssh(host: str, user: str, port: int, key_path: Optional[str], password: Optional[str],
                host_key_policy: str, retries: int = 3):
    try:
        import paramiko
    except ImportError as e:
        raise RuntimeError(
            "paramiko is required. Install it with: pip install paramiko"
        ) from e

    if host_key_policy == "strict":
        policy = paramiko.RejectPolicy()
    elif host_key_policy == "warn":
        policy = paramiko.WarningPolicy()
    else:
        policy = paramiko.AutoAddPolicy()

    last_err = None
    for attempt in range(1, retries + 1):
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(policy)
        # Load system host keys unless strict and no file exists (paramiko handles gracefully)
        try:
            ssh.load_system_host_keys()
        except Exception:
            pass

        try:
            pkey = None
            if key_path:
                pkey = paramiko.RSAKey.from_private_key_file(key_path)

            connect_kwargs = {
                "hostname": host,
                "port": port,
                "username": user,
                "timeout": 60,
                "banner_timeout": 60,
                "auth_timeout": 60,
                "look_for_keys": key_path is None,
            }
            if pkey:
                connect_kwargs["pkey"] = pkey
            elif password:
                connect_kwargs["password"] = password

            ssh.connect(**connect_kwargs)
            return ssh
        except Exception as e:
            last_err = e
            print("SSH connect attempt %d/%d failed: %s" % (attempt, retries, e), file=sys.stderr)
            if attempt < retries:
                time.sleep(5 * attempt)
        finally:
            if last_err is not None:
                try:
                    ssh.close()
                except Exception:
                    pass

    raise RuntimeError("Failed to connect to %s@%s:%d after %d attempts: %s" %
                       (user, host, port, retries, last_err))


def ssh_exec(ssh, cmd: str, echo: bool = True, timeout: int = 300, sensitive: bool = False):
    if echo:
        display_cmd = cmd if not sensitive else "<sensitive command redacted>"
        print("$ " + display_cmd)
    stdin, stdout, stderr = ssh.exec_command(cmd, timeout=timeout)
    exit_code = stdout.channel.recv_exit_status()
    out = stdout.read().decode("utf-8", errors="replace")
    err = stderr.read().decode("utf-8", errors="replace")
    if out:
        print(out)
    if err:
        print(err, file=sys.stderr)
    if exit_code != 0:
        raise RuntimeError("Command failed with exit code %d" % exit_code)
    return out


def upload_with_retries(host: str, user: str, port: int, key_path: Optional[str],
                        password: Optional[str], host_key_policy: str,
                        local: str, remote: str, retries: int = MAX_RETRIES):
    last_err = None
    for attempt in range(1, retries + 1):
        ssh = None
        sftp = None
        try:
            print("Upload %s -> %s (attempt %d/%d)" % (local, remote, attempt, retries))
            ssh = connect_ssh(host, user, port, key_path, password, host_key_policy)
            sftp = ssh.open_sftp()
            sftp.put(local, remote)
            return
        except Exception as e:
            last_err = e
            print("Upload attempt %d failed: %s" % (attempt, e), file=sys.stderr)
        finally:
            if sftp:
                try:
                    sftp.close()
                except Exception:
                    pass
            if ssh:
                try:
                    ssh.close()
                except Exception:
                    pass
    raise RuntimeError("Failed to upload %s after %d attempts: %s" % (local, retries, last_err))


# ---------------------------------------------------------------------------
# Archive helpers (lossy-network friendly)
# ---------------------------------------------------------------------------

def make_archive(mode: str, local_root: Path) -> str:
    archive = "/tmp/proxy-panel-prod-%s.tar.gz" % mode
    if mode == "hub":
        files = [
            ("target/release/proxy-panel-hub", "bin/proxy-panel-hub"),
            ("target/release/proxy-panel-agent", "bin/proxy-panel-agent"),
            ("target/release/proxy-panel", "bin/proxy-panel"),
            ("crates/pp-web/dist", "web/dist"),
            ("deploy/proxy-panel-hub.service", "service/proxy-panel-hub.service"),
            ("deploy/proxy-panel-agent.service", "service/proxy-panel-agent.service"),
        ]
    else:
        files = [
            ("target/release/proxy-panel-agent", "bin/proxy-panel-agent"),
            ("deploy/proxy-panel-agent.service", "service/proxy-panel-agent.service"),
        ]

    print("Creating %s..." % archive)
    with tarfile.open(archive, "w:gz") as tar:
        for local, arcname in files:
            local_path = local_root / local
            if not local_path.exists():
                raise FileNotFoundError("Missing deploy artifact: %s" % local_path)
            tar.add(local_path, arcname=arcname)
    size = os.path.getsize(archive)
    print("Archive size: %.1f MB" % (size / 1024 / 1024))
    return archive


def split_archive(archive: str) -> List[str]:
    chunks = []
    with open(archive, "rb") as f:
        idx = 0
        while True:
            data = f.read(CHUNK_SIZE)
            if not data:
                break
            chunk_path = "/tmp/proxy-panel-prod.chunk%03d" % idx
            with open(chunk_path, "wb") as out:
                out.write(data)
            chunks.append(chunk_path)
            idx += 1
    print("Split archive into %d chunks" % len(chunks))
    return chunks


def upload_archive(server, archive: str, remote_dir: str):
    chunks = split_archive(archive)
    ssh = connect_ssh(server.host, server.user, server.port, server.key_path,
                      server.password, server.host_key_policy)
    try:
        ssh_exec(ssh, "mkdir -p " + remote_dir)
    finally:
        ssh.close()

    for i, chunk in enumerate(chunks):
        remote_chunk = "%s/chunk%03d" % (remote_dir, i)
        upload_with_retries(server.host, server.user, server.port, server.key_path,
                            server.password, server.host_key_policy, chunk, remote_chunk)

    ssh = connect_ssh(server.host, server.user, server.port, server.key_path,
                      server.password, server.host_key_policy)
    try:
        ssh_exec(
            ssh,
            "cat %s/chunk* > /tmp/proxy-panel-prod.tar.gz && "
            "rm -rf %s/* && "
            "tar -xzf /tmp/proxy-panel-prod.tar.gz -C %s" % (remote_dir, remote_dir, remote_dir),
            timeout=120,
        )
    finally:
        ssh.close()


# ---------------------------------------------------------------------------
# Server model
# ---------------------------------------------------------------------------

@dataclass
class Server:
    name: str
    host: str
    mode: str  # "hub" or "agent"
    user: str
    port: int
    key_path: Optional[str]
    password: Optional[str]
    host_key_policy: str
    domain: Optional[str] = None
    hub_url: Optional[str] = None
    agent_token: Optional[str] = None
    agent_id: Optional[str] = None
    db_url: Optional[str] = None
    grpc_listen: str = "0.0.0.0:50052"
    listen: str = "127.0.0.1:8081"
    auto_register_agents: bool = False
    jwt_secret: Optional[str] = None
    admin_username: str = "admin"
    admin_password: Optional[str] = None
    cors_origins: Optional[str] = None
    install_caddy: bool = True
    tls_cert: Optional[str] = None
    tls_key: Optional[str] = None
    extra_env: Dict[str, str] = field(default_factory=dict)

    def require_hub(self):
        if self.mode != "hub":
            raise ValueError("Server %s is not configured as hub" % self.name)

    def require_agent(self):
        if self.mode not in ("hub", "agent"):
            raise ValueError("Server %s has invalid mode %s" % (self.name, self.mode))


def build_servers(inventory: dict) -> List[Server]:
    defaults = substitute_env_vars(inventory.get("defaults", {}))
    hub_defaults = substitute_env_vars(inventory.get("hub", {}))
    servers = []
    for raw in inventory.get("servers", []):
        raw = substitute_env_vars(raw)
        merged = {**defaults, **hub_defaults, **raw}

        mode = merged.get("mode")
        if mode not in ("hub", "agent"):
            raise ValueError("Server %s: mode must be 'hub' or 'agent'" % merged.get("name"))

        if mode == "hub" and not merged.get("domain"):
            raise ValueError("Server %s: domain is required for hub mode" % merged.get("name"))

        # Validate local agent configuration for hub servers.
        agent_cfg = merged.get("agent") if isinstance(merged.get("agent"), dict) else {}
        if mode == "hub" and agent_cfg.get("hub_url"):
            hub_url = agent_cfg["hub_url"]
            domain = merged.get("domain", "")
            if domain and domain in hub_url and "127.0.0.1" not in hub_url and "localhost" not in hub_url:
                raise ValueError(
                    "Server %s: local agent hub_url (%s) points to the public domain. "
                    "Use 'http://127.0.0.1:50052' for the local agent instead."
                    % (merged.get("name"), hub_url)
                )

        servers.append(Server(
            name=merged["name"],
            host=merged["host"],
            mode=mode,
            user=merged.get("user", "root"),
            port=int(merged.get("port", 22)),
            key_path=resolve_ssh_key(merged.get("ssh_key")),
            password=merged.get("ssh_password"),
            host_key_policy=merged.get("host_key_policy", "strict"),
            domain=merged.get("domain"),
            hub_url=agent_cfg.get("hub_url") if agent_cfg else merged.get("hub_url"),
            agent_token=agent_cfg.get("token") if agent_cfg else merged.get("agent_token"),
            agent_id=agent_cfg.get("agent_id") if agent_cfg else merged.get("agent_id"),
            db_url=merged.get("db_url", "sqlite:///opt/proxy-panel/data/proxypanel.db?mode=rwc"),
            grpc_listen=merged.get("grpc_listen", "0.0.0.0:50052"),
            listen=merged.get("listen", "127.0.0.1:8081"),
            auto_register_agents=bool(merged.get("auto_register_agents", False)),
            jwt_secret=merged.get("jwt_secret"),
            admin_username=merged.get("admin_username", "admin"),
            admin_password=merged.get("admin_password"),
            cors_origins=merged.get("cors_origins"),
            install_caddy=bool(merged.get("install_caddy", True)),
            tls_cert=merged.get("tls_cert"),
            tls_key=merged.get("tls_key"),
            extra_env=merged.get("extra_env", {}),
        ))
    return servers


# ---------------------------------------------------------------------------
# Remote commands
# ---------------------------------------------------------------------------

def remote_backup(ssh, server: Server) -> str:
    """Backup current binaries and web dist before update."""
    backup_id = time.strftime("%Y%m%d-%H%M%S")
    backup_path = "%s/%s-%s" % (REMOTE_BACKUP_DIR, server.name, backup_id)
    ssh_exec(ssh, "mkdir -p %s" % REMOTE_BACKUP_DIR)

    backup_cmds = [
        "cp -a %s/proxy-panel-hub %s/proxy-panel-hub 2>/dev/null || true" % (REMOTE_BIN, backup_path),
        "cp -a %s/proxy-panel-agent %s/proxy-panel-agent 2>/dev/null || true" % (REMOTE_BIN, backup_path),
        "cp -a %s/proxy-panel %s/proxy-panel 2>/dev/null || true" % (REMOTE_BIN, backup_path),
        "cp -a %s %s/dist 2>/dev/null || true" % (REMOTE_WEB, backup_path),
        "cp -a %s/hub.toml %s/hub.toml 2>/dev/null || true" % (REMOTE_ETC, backup_path),
        "cp -a %s/agent.env %s/agent.env 2>/dev/null || true" % (REMOTE_ETC, backup_path),
    ]
    ssh_exec(ssh, "mkdir -p %s && %s" % (backup_path, " && ".join(backup_cmds)), timeout=60)
    print("Backup created at %s" % backup_path)
    return backup_path


def remote_restore(ssh, server: Server, backup_path: str):
    """Restore binaries and config from backup."""
    print("!!! Restoring from %s" % backup_path)
    restore_cmds = [
        "cp -a %s/proxy-panel-hub %s/proxy-panel-hub 2>/dev/null || true" % (backup_path, REMOTE_BIN),
        "cp -a %s/proxy-panel-agent %s/proxy-panel-agent 2>/dev/null || true" % (backup_path, REMOTE_BIN),
        "cp -a %s/proxy-panel %s/proxy-panel 2>/dev/null || true" % (backup_path, REMOTE_BIN),
        "rm -rf %s && cp -a %s/dist %s 2>/dev/null || true" % (REMOTE_WEB, backup_path, REMOTE_WEB),
        "cp -a %s/hub.toml %s/hub.toml 2>/dev/null || true" % (backup_path, REMOTE_ETC),
        "cp -a %s/agent.env %s/agent.env 2>/dev/null || true" % (backup_path, REMOTE_ETC),
    ]
    ssh_exec(ssh, " && ".join(restore_cmds), timeout=60)


def install_caddy(ssh, server: Server):
    if not server.install_caddy:
        return
    print("\n=== Installing Caddy on %s ===" % server.name)
    ssh_exec(ssh, "apt-get update", timeout=180)
    ssh_exec(ssh, "apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl", timeout=180)
    ssh_exec(
        ssh,
        "curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg",
        timeout=60,
    )
    ssh_exec(
        ssh,
        "curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list",
        timeout=60,
    )
    ssh_exec(ssh, "apt-get update", timeout=180)
    ssh_exec(ssh, "apt-get install -y caddy", timeout=180)

    # gRPC upstream for agents: reverse proxy :443 -> hub:50052 with h2c
    caddyfile = """%s {
    reverse_proxy /proxypanel.HubAgent/* h2c://127.0.0.1:%d
    reverse_proxy 127.0.0.1:%d
}
""" % (server.domain, server.grpc_listen.rsplit(":", 1)[-1], server.listen.rsplit(":", 1)[-1])
    ssh_exec(ssh, "cat > /etc/caddy/Caddyfile <<'EOF'\n%sEOF" % caddyfile)
    ssh_exec(ssh, "systemctl enable caddy")


def write_hub_config(server: Server) -> str:
    cors = server.cors_origins or ("https://%s" % server.domain)
    jwt = server.jwt_secret or os.urandom(32).hex()
    config_lines = [
        'listen = "%s"' % server.listen,
        'grpc_listen = "%s"' % server.grpc_listen,
        'database_url = "%s"' % server.db_url,
        'static_dir = "%s"' % REMOTE_WEB,
        'cors_origins = "%s"' % cors,
        'trusted_proxy_ips = "127.0.0.1,::1"',
        'auto_register_agents = %s' % ("true" if server.auto_register_agents else "false"),
        'jwt_secret = "%s"' % jwt,
    ]
    return "\n".join(config_lines) + "\n"


def write_hub_env(server: Server) -> str:
    env = ["RUST_LOG=proxy_panel_hub=info,tower_http=info"]
    for k, v in server.extra_env.items():
        env.append("%s=%s" % (k, v))
    return "\n".join(env) + "\n"


def write_agent_env(server: Server) -> str:
    env = [
        "RUST_LOG=proxy_panel_agent=info",
        "PROXYPANEL_HUB_URL=%s" % server.hub_url,
        "PROXYPANEL_AGENT_TOKEN=%s" % server.agent_token,
    ]
    for k, v in server.extra_env.items():
        env.append("%s=%s" % (k, v))
    return "\n".join(env) + "\n"


def provision_agent(ssh, server: Server) -> Tuple[str, str]:
    """Provision a new node on the hub and return (node_id, token)."""
    print("\n=== Provisioning agent node on hub ===")
    cmd = (
        "%s/proxy-panel provision-node --database-url '%s' --name '%s' --hostname '%s' --address '%s'"
        % (REMOTE_BIN, server.db_url, server.name, server.host, server.host)
    )
    out = ssh_exec(ssh, cmd, timeout=60)
    node_id = None
    token = None
    for line in out.splitlines():
        if line.startswith("node_id:"):
            node_id = line.split(":", 1)[1].strip()
        elif line.startswith("token:"):
            token = line.split(":", 1)[1].strip()
    if not node_id or not token:
        raise RuntimeError("Failed to provision agent: %s" % out)
    return node_id, token


def deploy_hub(ssh, server: Server) -> Dict[str, str]:
    server.require_hub()
    print("\n=== Deploying Hub on %s ===" % server.name)

    admin_password = server.admin_password or os.urandom(16).hex()

    ssh_exec(
        ssh,
        "mkdir -p %s %s %s %s %s %s /var/lib/proxy-panel/agent /tmp/proxy-panel-prod"
        % (REMOTE_DIR + "/bin", REMOTE_DIR + "/config", REMOTE_WEB, REMOTE_DATA, REMOTE_AGENT_BIN, REMOTE_ETC),
    )

    archive = make_archive("hub", LOCAL_ROOT)
    upload_archive(server, archive, "/tmp/proxy-panel-prod")

    print("\n=== Writing Hub configuration ===")
    hub_config = write_hub_config(server)
    hub_env = write_hub_env(server)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
        f.write(hub_config)
        hub_config_local = f.name
    with tempfile.NamedTemporaryFile(mode="w", suffix=".env", delete=False) as f:
        f.write(hub_env)
        hub_env_local = f.name

    upload_with_retries(server.host, server.user, server.port, server.key_path,
                        server.password, server.host_key_policy,
                        hub_config_local, REMOTE_ETC + "/hub.toml")
    upload_with_retries(server.host, server.user, server.port, server.key_path,
                        server.password, server.host_key_policy,
                        hub_env_local, REMOTE_ETC + "/hub.env")

    install_caddy(ssh, server)

    install_script = """#!/bin/bash
set -euo pipefail
ARCHIVE_DIR=/tmp/proxy-panel-prod

useradd -r -s /bin/false proxypanel 2>/dev/null || true

install -m 755 "$ARCHIVE_DIR/bin/proxy-panel-hub" %s/proxy-panel-hub
install -m 755 "$ARCHIVE_DIR/bin/proxy-panel-agent" %s/proxy-panel-agent
install -m 755 "$ARCHIVE_DIR/bin/proxy-panel" %s/proxy-panel

rm -rf %s
mkdir -p %s
cp -a "$ARCHIVE_DIR/web/dist/." %s/

cp "$ARCHIVE_DIR/service/proxy-panel-hub.service" /etc/systemd/system/proxy-panel-hub.service
cp "$ARCHIVE_DIR/service/proxy-panel-agent.service" /etc/systemd/system/proxy-panel-agent.service

chown -R proxypanel:proxypanel %s /var/lib/proxy-panel %s
chmod 640 %s/hub.toml %s/hub.env

%s/proxy-panel init-db --database-url '%s'
%s/proxy-panel create-user --database-url '%s' --username '%s' --password '%s'

# Fix SQLite ownership so the hub service can write
chown -R proxypanel:proxypanel %s

systemctl daemon-reload
systemctl enable --now proxy-panel-hub
sleep 2
systemctl enable --now proxy-panel-agent
sleep 5

systemctl is-active proxy-panel-hub proxy-panel-agent
""" % (
        REMOTE_BIN, REMOTE_BIN, REMOTE_BIN,
        REMOTE_WEB, REMOTE_WEB, REMOTE_WEB,
        REMOTE_DIR, REMOTE_ETC, REMOTE_ETC, REMOTE_ETC,
        REMOTE_BIN, server.db_url,
        REMOTE_BIN, server.db_url, server.admin_username, admin_password,
        REMOTE_DATA,
    )

    with tempfile.NamedTemporaryFile(mode="w", suffix=".sh", delete=False) as f:
        f.write(install_script)
        install_local = f.name

    upload_with_retries(server.host, server.user, server.port, server.key_path,
                        server.password, server.host_key_policy,
                        install_local, "/tmp/proxy-panel-prod-install.sh")
    ssh_exec(ssh, "bash /tmp/proxy-panel-prod-install.sh", timeout=300)

    if server.install_caddy:
        ssh_exec(ssh, "systemctl restart caddy", timeout=60)

    # Health check
    health = ssh_exec(
        ssh,
        "curl -sfk https://127.0.0.1/health -H 'Host: %s' || curl -sf http://127.0.0.1:%s/health || echo HEALTH_CHECK_FAILED"
        % (server.domain, server.listen.rsplit(":", 1)[-1]),
        timeout=30,
    )
    print("Health check:", health.strip())

    # Provision local agent if no explicit token/id provided
    if not server.agent_token or not server.agent_id:
        node_id, token = provision_agent(ssh, server)
        agent_env = write_agent_env(Server(
            name=server.name, host=server.host, mode="hub",
            user=server.user, port=server.port, key_path=server.key_path,
            password=server.password, host_key_policy=server.host_key_policy,
            hub_url="http://127.0.0.1:%s" % server.grpc_listen.rsplit(":", 1)[-1],
            agent_token=token,
            extra_env=server.extra_env,
        ))
        with tempfile.NamedTemporaryFile(mode="w", suffix=".env", delete=False) as f:
            f.write(agent_env)
            agent_env_local = f.name
        upload_with_retries(server.host, server.user, server.port, server.key_path,
                            server.password, server.host_key_policy,
                            agent_env_local, REMOTE_ETC + "/agent.env")
        # Update service file to include agent-id for local agent
        ssh_exec(ssh, (
            "sed -i 's|--hub-url ${PROXYPANEL_HUB_URL}|--agent-id %s --hub-url ${PROXYPANEL_HUB_URL}|' "
            "/etc/systemd/system/proxy-panel-agent.service"
        ) % node_id)
        ssh_exec(ssh, "systemctl daemon-reload && systemctl restart proxy-panel-agent", timeout=60)
    else:
        node_id = server.agent_id
        token = server.agent_token

    for p in [hub_config_local, hub_env_local, install_local]:
        try:
            os.remove(p)
        except OSError:
            pass
    try:
        os.remove(agent_env_local)
    except (NameError, OSError):
        pass

    return {
        "admin_password": admin_password,
        "node_id": node_id,
        "agent_token": token,
    }


def deploy_agent(ssh, server: Server):
    server.require_agent()
    if not server.hub_url or not server.agent_token:
        raise ValueError("Server %s: agent.hub_url and agent.token are required" % server.name)

    print("\n=== Deploying Agent on %s ===" % server.name)

    ssh_exec(
        ssh,
        "mkdir -p %s %s %s /var/lib/proxy-panel/agent /tmp/proxy-panel-prod"
        % (REMOTE_DIR, REMOTE_AGENT_BIN, REMOTE_ETC),
    )

    archive = make_archive("agent", LOCAL_ROOT)
    upload_archive(server, archive, "/tmp/proxy-panel-prod")

    agent_env = write_agent_env(server)
    with tempfile.NamedTemporaryFile(mode="w", suffix=".env", delete=False) as f:
        f.write(agent_env)
        agent_env_local = f.name

    upload_with_retries(server.host, server.user, server.port, server.key_path,
                        server.password, server.host_key_policy,
                        agent_env_local, REMOTE_ETC + "/agent.env")

    install_script = """#!/bin/bash
set -euo pipefail
ARCHIVE_DIR=/tmp/proxy-panel-prod

useradd -r -s /bin/false proxypanel 2>/dev/null || true

install -m 755 "$ARCHIVE_DIR/bin/proxy-panel-agent" %s/proxy-panel-agent
cp "$ARCHIVE_DIR/service/proxy-panel-agent.service" /etc/systemd/system/proxy-panel-agent.service

chown -R proxypanel:proxypanel %s /var/lib/proxy-panel %s
chmod 640 %s/agent.env

systemctl daemon-reload
systemctl enable --now proxy-panel-agent
sleep 5

systemctl is-active proxy-panel-agent
""" % (REMOTE_BIN, REMOTE_DIR, REMOTE_ETC, REMOTE_ETC)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".sh", delete=False) as f:
        f.write(install_script)
        install_local = f.name

    upload_with_retries(server.host, server.user, server.port, server.key_path,
                        server.password, server.host_key_policy,
                        install_local, "/tmp/proxy-panel-prod-install.sh")
    ssh_exec(ssh, "bash /tmp/proxy-panel-prod-install.sh", timeout=120)

    for p in [agent_env_local, install_local]:
        try:
            os.remove(p)
        except OSError:
            pass


def update_hub(ssh, server: Server):
    server.require_hub()
    print("\n=== Updating Hub on %s ===" % server.name)
    backup_path = remote_backup(ssh, server)

    try:
        archive = make_archive("hub", LOCAL_ROOT)
        upload_archive(server, archive, "/tmp/proxy-panel-prod")

        ssh_exec(ssh, "systemctl stop proxy-panel-hub proxy-panel-agent", timeout=60)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-prod/bin/proxy-panel-hub %s/proxy-panel-hub" % REMOTE_BIN)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-prod/bin/proxy-panel-agent %s/proxy-panel-agent" % REMOTE_BIN)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-prod/bin/proxy-panel %s/proxy-panel" % REMOTE_BIN)
        ssh_exec(ssh, "rm -rf %s && mkdir -p %s && cp -a /tmp/proxy-panel-prod/web/dist/. %s/" % (REMOTE_WEB, REMOTE_WEB, REMOTE_WEB))
        ssh_exec(ssh, "chown -R proxypanel:proxypanel %s /var/lib/proxy-panel %s" % (REMOTE_DIR, REMOTE_ETC))

        # Run migrations
        ssh_exec(ssh, "%s/proxy-panel init-db --database-url '%s'" % (REMOTE_BIN, server.db_url), timeout=120)

        ssh_exec(ssh, "systemctl daemon-reload")
        ssh_exec(ssh, "systemctl start proxy-panel-hub", timeout=60)
        time.sleep(3)
        ssh_exec(ssh, "systemctl start proxy-panel-agent", timeout=60)
        time.sleep(5)
        ssh_exec(ssh, "systemctl is-active proxy-panel-hub proxy-panel-agent")

        health = ssh_exec(
            ssh,
            "curl -sfk https://127.0.0.1/health -H 'Host: %s' || curl -sf http://127.0.0.1:%s/health || echo HEALTH_CHECK_FAILED"
            % (server.domain, server.listen.rsplit(":", 1)[-1]),
            timeout=30,
        )
        print("Health check:", health.strip())
    except Exception as e:
        print("Update failed on %s: %s" % (server.name, e), file=sys.stderr)
        print("Rolling back...", file=sys.stderr)
        remote_restore(ssh, server, backup_path)
        ssh_exec(ssh, "systemctl daemon-reload && systemctl start proxy-panel-hub proxy-panel-agent || true", timeout=60)
        raise


def update_agent(ssh, server: Server):
    server.require_agent()
    print("\n=== Updating Agent on %s ===" % server.name)
    backup_path = remote_backup(ssh, server)

    try:
        archive = make_archive("agent", LOCAL_ROOT)
        upload_archive(server, archive, "/tmp/proxy-panel-prod")

        ssh_exec(ssh, "systemctl stop proxy-panel-agent", timeout=60)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-prod/bin/proxy-panel-agent %s/proxy-panel-agent" % REMOTE_BIN)
        ssh_exec(ssh, "chown -R proxypanel:proxypanel %s /var/lib/proxy-panel %s" % (REMOTE_DIR, REMOTE_ETC))
        ssh_exec(ssh, "systemctl daemon-reload")
        ssh_exec(ssh, "systemctl start proxy-panel-agent", timeout=60)
        time.sleep(5)
        ssh_exec(ssh, "systemctl is-active proxy-panel-agent")
    except Exception as e:
        print("Update failed on %s: %s" % (server.name, e), file=sys.stderr)
        print("Rolling back...", file=sys.stderr)
        remote_restore(ssh, server, backup_path)
        ssh_exec(ssh, "systemctl daemon-reload && systemctl start proxy-panel-agent || true", timeout=60)
        raise


def run_action(server: Server, action: str) -> Optional[Dict[str, str]]:
    ssh = connect_ssh(server.host, server.user, server.port, server.key_path,
                      server.password, server.host_key_policy)
    try:
        if action == "deploy":
            if server.mode == "hub":
                return deploy_hub(ssh, server)
            return deploy_agent(ssh, server)
        if action == "update":
            if server.mode == "hub":
                update_hub(ssh, server)
            else:
                update_agent(ssh, server)
            return None
        raise ValueError("Unknown action: %s" % action)
    finally:
        ssh.close()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Deploy or update ProxyPanel in production")
    parser.add_argument("--inventory", "-i", required=True, help="Path to YAML or JSON inventory file")
    parser.add_argument("--action", "-a", required=True, choices=["deploy", "update"],
                        help="deploy = fresh install, update = replace binaries with rollback")
    parser.add_argument("--limit", "-l", default="", help="Comma-separated list of server names to target")
    parser.add_argument("--parallel", "-p", action="store_true", help="Run operations in parallel (experimental)")
    parser.add_argument("--host-key-policy", default="strict", choices=["strict", "warn", "auto"],
                        help="SSH host key verification policy (default: strict)")
    parser.add_argument("--secrets-out", "-s", default="",
                        help="Write generated secrets (admin_password, tokens) to this file")
    args = parser.parse_args()

    inventory = load_inventory(args.inventory)
    servers = build_servers(inventory)

    if args.limit:
        allowed = {n.strip() for n in args.limit.split(",")}
        servers = [s for s in servers if s.name in allowed]
        if not servers:
            raise ValueError("No servers matched --limit=%s" % args.limit)

    # Apply global host-key policy override unless per-server is set
    for s in servers:
        if s.host_key_policy == "strict" and args.host_key_policy != "strict":
            s.host_key_policy = args.host_key_policy

    results = {}
    if args.parallel:
        from concurrent.futures import ThreadPoolExecutor, as_completed
        with ThreadPoolExecutor(max_workers=min(len(servers), 4)) as executor:
            futures = {executor.submit(run_action, s, args.action): s for s in servers}
            for future in as_completed(futures):
                server = futures[future]
                try:
                    result = future.result()
                    results[server.name] = result
                    print("\n=== %s completed successfully ===" % server.name)
                except Exception as e:
                    print("\n=== %s FAILED: %s ===" % (server.name, e), file=sys.stderr)
    else:
        for server in servers:
            try:
                result = run_action(server, args.action)
                results[server.name] = result
                print("\n=== %s completed successfully ===" % server.name)
            except Exception as e:
                print("\n=== %s FAILED: %s ===" % (server.name, e), file=sys.stderr)

    # Print summary
    print("\n" + "=" * 60)
    print("DEPLOYMENT SUMMARY")
    print("=" * 60)
    secrets_lines = ["# ProxyPanel generated secrets - store securely and delete after reading\n"]
    for server in servers:
        result = results.get(server.name)
        if isinstance(result, dict):
            print("\n%s (hub):" % server.name)
            print("  Panel URL:      https://%s" % server.domain)
            print("  Admin user:     %s" % server.admin_username)
            if "admin_password" in result:
                print("  Admin password: %s" % result["admin_password"])
                secrets_lines.append("%s_ADMIN_PASSWORD=%s" % (server.name.upper(), result["admin_password"]))
            if "agent_token" in result and "node_id" in result:
                print("  Local agent token: %s" % result["agent_token"])
                print("  Local agent id:    %s" % result["node_id"])
                secrets_lines.append("%s_AGENT_TOKEN=%s" % (server.name.upper(), result["agent_token"]))
                secrets_lines.append("%s_AGENT_ID=%s" % (server.name.upper(), result["node_id"]))
        else:
            print("\n%s (%s): OK" % (server.name, server.mode))

    if args.secrets_out and secrets_lines:
        with open(args.secrets_out, "w", encoding="utf-8") as f:
            f.write("\n".join(secrets_lines) + "\n")
        print("\nGenerated secrets written to: %s" % args.secrets_out)
        print("IMPORTANT: This file contains plaintext secrets. Store it securely and delete it when no longer needed.")


if __name__ == "__main__":
    main()
