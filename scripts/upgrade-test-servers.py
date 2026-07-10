#!/usr/bin/env python3
"""In-place upgrade ProxyPanel on test2/test3 over lossy networks.

Does NOT wipe the database or recreate users/agents. It only replaces
binaries and the web frontend, then restarts services.

Servers:
  test3 (hub + local agent): 64.188.27.110
  test2 (remote agent):      192.3.150.233
"""

import argparse
import os
import sys
import tarfile
import tempfile
import time
import paramiko

CHUNK_SIZE = 1024 * 1024  # 1 MiB
MAX_RETRIES = 5

LOCAL_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REMOTE_DIR = "/opt/proxy-panel"
REMOTE_BIN = "/usr/local/bin"
REMOTE_ETC = "/etc/proxy-panel"
REMOTE_WEB = REMOTE_DIR + "/web/dist"


def connect_ssh(host, password, port=22, retries=3):
    last_err = None
    for attempt in range(1, retries + 1):
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        try:
            ssh.connect(
                host,
                port=port,
                username="root",
                password=password,
                timeout=60,
                banner_timeout=60,
                auth_timeout=60,
            )
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
    raise RuntimeError("Failed to connect to %s after %d attempts: %s" % (host, retries, last_err))


def ssh_exec(ssh, cmd, echo=True, timeout=120):
    if echo:
        print("$ " + cmd)
    stdin, stdout, stderr = ssh.exec_command(cmd, timeout=timeout)
    exit_code = stdout.channel.recv_exit_status()
    out = stdout.read().decode("utf-8", errors="replace")
    err = stderr.read().decode("utf-8", errors="replace")
    if out:
        print(out)
    if err:
        print(err, file=sys.stderr)
    if exit_code != 0:
        raise RuntimeError("Command failed with exit code %d: %s" % (exit_code, cmd))
    return out


def upload_with_retries(host, password, local, remote, retries=MAX_RETRIES):
    last_err = None
    for attempt in range(1, retries + 1):
        ssh = None
        sftp = None
        try:
            print("Upload %s -> %s (attempt %d/%d)" % (local, remote, attempt, retries))
            ssh = connect_ssh(host, password)
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


def make_archive(mode):
    archive = "/tmp/proxy-panel-upgrade-%s.tar.gz" % mode
    if mode == "hub":
        files = [
            ("target/release/proxy-panel-hub", "bin/proxy-panel-hub"),
            ("target/release/proxy-panel-agent", "bin/proxy-panel-agent"),
            ("target/release/proxy-panel", "bin/proxy-panel"),
            ("crates/pp-web/dist", "web/dist"),
        ]
    else:
        files = [
            ("target/release/proxy-panel-agent", "bin/proxy-panel-agent"),
        ]
    print("Creating %s..." % archive)
    with tarfile.open(archive, "w:gz") as tar:
        for local, arcname in files:
            local_path = os.path.join(LOCAL_ROOT, local)
            if not os.path.exists(local_path):
                raise FileNotFoundError("Missing deploy artifact: " + local_path)
            tar.add(local_path, arcname=arcname)
    size = os.path.getsize(archive)
    print("Archive size: %.1f MB" % (size / 1024 / 1024))
    return archive


def split_archive(archive):
    chunks = []
    with open(archive, "rb") as f:
        idx = 0
        while True:
            data = f.read(CHUNK_SIZE)
            if not data:
                break
            chunk_path = "/tmp/proxy-panel-upgrade.chunk%03d" % idx
            with open(chunk_path, "wb") as out:
                out.write(data)
            chunks.append(chunk_path)
            idx += 1
    print("Split archive into %d chunks" % len(chunks))
    return chunks


def upload_archive(host, password, archive, remote_dir):
    chunks = split_archive(archive)
    ssh = connect_ssh(host, password)
    try:
        ssh_exec(ssh, "mkdir -p " + remote_dir)
    finally:
        ssh.close()
    for i, chunk in enumerate(chunks):
        remote_chunk = "%s/chunk%03d" % (remote_dir, i)
        upload_with_retries(host, password, chunk, remote_chunk)
    ssh = connect_ssh(host, password)
    try:
        ssh_exec(
            ssh,
            "cat %s/chunk* > /tmp/proxy-panel-upgrade.tar.gz && rm -rf %s/* && tar -xzf /tmp/proxy-panel-upgrade.tar.gz -C %s" % (remote_dir, remote_dir, remote_dir),
        )
    finally:
        ssh.close()


def upgrade_hub(host, password):
    print("\n=== Upgrading Hub on %s ===" % host)
    archive = make_archive("hub")
    upload_archive(host, password, archive, "/tmp/proxy-panel-upgrade")

    ssh = connect_ssh(host, password)
    try:
        ssh_exec(ssh, "systemctl stop proxy-panel-hub proxy-panel-agent", timeout=60)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-upgrade/bin/proxy-panel-hub %s/proxy-panel-hub" % REMOTE_BIN)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-upgrade/bin/proxy-panel-agent %s/proxy-panel-agent" % REMOTE_BIN)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-upgrade/bin/proxy-panel %s/proxy-panel" % REMOTE_BIN)
        ssh_exec(ssh, "rm -rf %s && mkdir -p %s && cp -a /tmp/proxy-panel-upgrade/web/dist/. %s/" % (REMOTE_WEB, REMOTE_WEB, REMOTE_WEB))
        ssh_exec(ssh, "chown -R proxypanel:proxypanel %s /var/lib/proxy-panel %s" % (REMOTE_DIR, REMOTE_ETC))
        ssh_exec(ssh, "systemctl daemon-reload")
        ssh_exec(ssh, "systemctl start proxy-panel-hub")
        time.sleep(3)
        ssh_exec(ssh, "systemctl start proxy-panel-agent")
        time.sleep(5)
        ssh_exec(ssh, "systemctl status proxy-panel-hub --no-pager")
        ssh_exec(ssh, "systemctl status proxy-panel-agent --no-pager")
    finally:
        ssh.close()


def upgrade_agent(host, password):
    print("\n=== Upgrading Agent on %s ===" % host)
    archive = make_archive("agent")
    upload_archive(host, password, archive, "/tmp/proxy-panel-upgrade")

    ssh = connect_ssh(host, password)
    try:
        ssh_exec(ssh, "systemctl stop proxy-panel-agent", timeout=60)
        ssh_exec(ssh, "install -m 755 /tmp/proxy-panel-upgrade/bin/proxy-panel-agent %s/proxy-panel-agent" % REMOTE_BIN)
        ssh_exec(ssh, "systemctl daemon-reload")
        ssh_exec(ssh, "systemctl start proxy-panel-agent")
        time.sleep(5)
        ssh_exec(ssh, "systemctl status proxy-panel-agent --no-pager")
    finally:
        ssh.close()


def main():
    parser = argparse.ArgumentParser(description="In-place upgrade ProxyPanel on test servers")
    parser.add_argument("--target", required=True, choices=["test3", "test2", "all"])
    parser.add_argument("--test3-host", default=os.environ.get("TEST3_HOST", "64.188.27.110"))
    parser.add_argument("--test3-password", default=os.environ.get("TEST3_PASSWORD", ""))
    parser.add_argument("--test2-host", default=os.environ.get("TEST2_HOST", "192.3.150.233"))
    parser.add_argument("--test2-password", default=os.environ.get("TEST2_PASSWORD", ""))
    args = parser.parse_args()

    if args.target in ("test3", "all"):
        if not args.test3_password:
            parser.error("--test3-password or TEST3_PASSWORD required")
        upgrade_hub(args.test3_host, args.test3_password)

    if args.target in ("test2", "all"):
        if not args.test2_password:
            parser.error("--test2-password or TEST2_PASSWORD required")
        upgrade_agent(args.test2_host, args.test2_password)

    print("\n=== Upgrade complete ===")


if __name__ == "__main__":
    main()
