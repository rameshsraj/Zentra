#!/usr/bin/env python3
"""
Comprehensive smoke test for every Zentra API GET endpoint.

For each endpoint:
- Substitute :param style placeholders with sentinel UUID / id
- Call with Bearer JWT auth
- Categorize: OK (2xx), CLIENT (4xx — expected for sentinel ids), SERVER (5xx — bug), TIMEOUT
- Flag empty-but-OK responses ([]/{}/null) so we know which endpoints have no data

Usage:
  BASE_URL=http://localhost:5006 \
  EMAIL=admin@Zentra.local PASSWORD=changeme \
  python3 smoke_test_all.py < endpoints.txt
"""
import json
import os
import re
import sys
import time
import urllib.request
import urllib.error
from collections import defaultdict

BASE = os.environ.get("BASE_URL", "http://localhost:5006").rstrip("/")
EMAIL = os.environ.get("EMAIL", "admin@Zentra.local")
PASSWORD = os.environ.get("PASSWORD", "changeme")
TIMEOUT = float(os.environ.get("TIMEOUT", "5"))

SENTINEL_UUID = "00000000-0000-0000-0000-000000000001"
SENTINEL_INT = "1"
SENTINEL_NAME = "test"


def http_request(method, path, token=None, body=None, timeout=TIMEOUT):
    url = f"{BASE}{path}"
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read().decode(errors="replace")
    except urllib.error.HTTPError as e:
        try:
            body = e.read().decode(errors="replace")
        except Exception:
            body = ""
        return e.code, body
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        return -1, str(e)


def login():
    code, body = http_request(
        "POST", "/api/auth/login", body={"email": EMAIL, "password": PASSWORD}
    )
    if code != 200:
        sys.exit(f"LOGIN FAILED ({code}): {body}")
    return json.loads(body)["token"]


def substitute_params(path):
    """Replace :name placeholders with sentinel values."""
    def repl(m):
        name = m.group(1).lower()
        if "id" in name or "uuid" in name or "key" in name:
            return SENTINEL_UUID
        if "name" in name or "type" in name:
            return SENTINEL_NAME
        if "timestamp" in name or "time" in name:
            return "1700000000"
        return SENTINEL_INT
    return re.sub(r":(\w+)", repl, path)


def is_empty_body(body):
    if not body:
        return True
    try:
        v = json.loads(body)
        if v is None or v == [] or v == {}:
            return True
        if isinstance(v, dict):
            # common wrappers like {"items": [], "total": 0}
            if all(
                (k in ("items", "results", "data", "rows") and v[k] in ([], None))
                or (k in ("total", "count") and v[k] == 0)
                for k in v
            ):
                return True
        return False
    except Exception:
        return False


def main():
    # Read endpoints from stdin (lines like "get /api/foo/:id")
    endpoints = []
    for line in sys.stdin:
        line = line.strip()
        if not line or " " not in line:
            continue
        method, path = line.split(None, 1)
        method = method.upper()
        path = path.strip()
        if not path.startswith("/"):
            continue
        endpoints.append((method, path))

    token = login()
    print(f"Logged in. Testing {len(endpoints)} endpoints against {BASE}\n", flush=True)

    buckets = defaultdict(list)
    empty_ok = []
    start = time.time()

    for i, (method, path) in enumerate(endpoints, 1):
        # Skip non-GETs — they often need bodies / would mutate state
        if method != "GET":
            buckets["SKIP_NON_GET"].append((method, path))
            continue

        # Skip WebSocket-style upgrade endpoints
        if path.endswith("/live") or path.endswith("/tail") or "ws" in path:
            buckets["SKIP_WS"].append((method, path))
            continue

        # Skip endpoints that need body parameters via query string we don't know
        actual_path = substitute_params(path)
        code, body = http_request("GET", actual_path, token=token)

        if code == -1:
            buckets["NETWORK_ERR"].append((path, body[:120]))
        elif code == 501:
            buckets["NOT_IMPLEMENTED"].append((path, body[:120]))
        elif code >= 500:
            buckets["SERVER_5XX"].append((path, code, body[:200]))
        elif 200 <= code < 300:
            buckets["OK_2XX"].append((path, code))
            if is_empty_body(body):
                empty_ok.append((path, body[:80]))
        elif 400 <= code < 500:
            buckets[f"CLIENT_{code}"].append(path)
        else:
            buckets[f"OTHER_{code}"].append(path)

        if i % 50 == 0:
            print(f"  ... {i}/{len(endpoints)} ({time.time()-start:.1f}s)", flush=True)

    elapsed = time.time() - start
    print(f"\n=== RESULTS ({elapsed:.1f}s) ===\n")

    def n(k): return len(buckets[k])
    print(f"  OK (2xx):              {n('OK_2XX')}")
    print(f"    └─ Empty payload:    {len(empty_ok)}")
    print(f"  CLIENT 401:            {n('CLIENT_401')}")
    print(f"  CLIENT 403:            {n('CLIENT_403')}")
    print(f"  CLIENT 404:            {n('CLIENT_404')}")
    print(f"  CLIENT 400/422:        {n('CLIENT_400') + n('CLIENT_422')}")
    print(f"  Not Implemented 501:   {n('NOT_IMPLEMENTED')}  (intentional future-work)")
    print(f"  *** SERVER 5xx ***:    {n('SERVER_5XX')}")
    print(f"  Network errors:        {n('NETWORK_ERR')}")
    print(f"  Skipped (non-GET):     {n('SKIP_NON_GET')}")
    print(f"  Skipped (ws/live):     {n('SKIP_WS')}")

    if buckets["SERVER_5XX"]:
        print("\n*** 5xx ERRORS — REAL BUGS ***")
        for path, code, body in buckets["SERVER_5XX"]:
            print(f"  [{code}] {path}")
            print(f"        {body}")

    if buckets["NETWORK_ERR"]:
        print("\n*** NETWORK ERRORS ***")
        for path, err in buckets["NETWORK_ERR"][:10]:
            print(f"  {path}: {err}")

    if empty_ok:
        print(f"\n*** EMPTY RESPONSES ({len(empty_ok)}) ***")
        for path, body in empty_ok[:30]:
            print(f"  {path}  →  {body}")
        if len(empty_ok) > 30:
            print(f"  ... and {len(empty_ok)-30} more")

    # Final verdict
    print()
    if buckets["SERVER_5XX"] or buckets["NETWORK_ERR"]:
        print("VERDICT: FAIL — server errors found")
        sys.exit(1)
    print("VERDICT: PASS — no 5xx errors")


if __name__ == "__main__":
    main()
