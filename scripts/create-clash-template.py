#!/usr/bin/env python3
"""Create the Clash full-config subscription template on a remote Hub.

Usage:
    export PANEL_API_KEY=your_api_key
    export PANEL_HOST=https://test3-panel.ybakiame.net
    python3 scripts/create-clash-template.py

The script will create (or update if a template named 'clash-full' already exists)
the Clash Meta / Mihomo full-config template using the placeholder mechanism
supported by pp-subscription:
    - "<PROXY_REPLACE>" -> proxy list
    - "<NODE_REPLACE>"  -> proxy name list
"""

import json
import os
import sys
import urllib.request
import urllib.error

HOST = os.environ.get("PANEL_HOST", "https://test3-panel.ybakiame.net").rstrip("/")
API_KEY = os.environ.get("PANEL_API_KEY", "")
TEMPLATE_PATH = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "deploy",
    "clash-full-template.json",
)
TEMPLATE_NAME = "clash-full"


def api_request(method, path, body=None):
    url = f"{HOST}{path}"
    data = json.dumps(body).encode("utf-8") if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"Bearer {API_KEY}")
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        err = e.read().decode("utf-8", errors="replace")
        print(f"HTTP {e.code}: {err}", file=sys.stderr)
        raise


def main():
    if not API_KEY:
        print("ERROR: PANEL_API_KEY environment variable is required", file=sys.stderr)
        sys.exit(1)

    with open(TEMPLATE_PATH, "r", encoding="utf-8") as f:
        base_config = json.load(f)

    # Find existing template
    existing_id = None
    page = 1
    while True:
        resp = api_request("GET", f"/api/v1/templates?page={page}&per_page=100")
        for t in resp.get("data", []):
            if t.get("name") == TEMPLATE_NAME:
                existing_id = t["id"]
                break
        if existing_id or page >= (resp.get("pagination", {}).get("total_pages", 1)):
            break
        page += 1

    payload = {
        "name": TEMPLATE_NAME,
        "format": "clash",
        "base_config": base_config,
        "filter_rules": {},
        "custom_headers": {},
    }

    if existing_id:
        api_request("DELETE", f"/api/v1/templates/{existing_id}")
        print(f"Removed old template '{TEMPLATE_NAME}' ({existing_id})")

    result = api_request("POST", "/api/v1/templates", payload)
    template_id = result.get("data", result).get("id")
    print(f"Created template '{TEMPLATE_NAME}' ({template_id})")


if __name__ == "__main__":
    main()
