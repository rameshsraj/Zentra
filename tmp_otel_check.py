import json
import os
import subprocess
import urllib.error
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:5006"


def http(method, path, token=None, body=None):
    data = json.dumps(body).encode() if body is not None else None
    headers = {"accept": "application/json"}
    if body is not None:
        headers["content-type"] = "application/json"
    if token:
        headers["authorization"] = f"Bearer {token}"
    req = urllib.request.Request(BASE + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read().decode("utf-8", "replace")


def compact(value):
    try:
        parsed = json.loads(value)
        return json.dumps(parsed, separators=(",", ":"), sort_keys=True)[:700]
    except Exception:
        return value.replace("\n", " ")[:700]


def psql(sql):
    return subprocess.check_output(
        [
            "psql",
            "-h",
            "127.0.0.1",
            "-p",
            "5434",
            "-U",
            "postgres",
            "-d",
            "zentra",
            "-Atc",
            sql,
        ],
        env={**os.environ, "PGPASSWORD": "zantaz"},
        text=True,
    ).strip()


code, login = http("POST", "/api/auth/login", body={"email": "admin@Zentra.local", "password": "changeme"})
payload = json.loads(login)
token = payload.get("access_token") or payload.get("token")
print(f"LOGIN {code} token={'yes' if token else 'no'}")

paths = [
    "/api/otel/status",
    "/api/otel/collector-config",
    "/api/otel/resources",
    "/api/otel/stats",
    "/api/otel/stats/timeseries",
    "/api/otel/pipeline",
    "/api/otel/span-metrics",
    "/api/otel/alert-rules",
    "/api/otel/alert-rules/evaluate",
    "/api/otel/genai",
    "/api/otel/api-keys",
    "/api/otel/ingest-audit",
    "/api/otel/trace-aggregates",
    "/api/otel/attributes?signal=traces",
    "/api/otel/attributes?signal=logs",
    "/api/otel/semconv",
    "/api/logs?limit=5",
    "/api/logs/query?limit=5",
    "/api/logs/streams",
    "/api/logs/volume",
]

for path in paths:
    code, body = http("GET", path, token=token)
    print(f"HTTP {code} {path} {compact(body)}")

queries = {
    "traces_total": "select count(*) from traces;",
    "traces_24h": "select count(*) from traces where started_at > now() - interval '24 hours';",
    "metrics_total": "select count(*) from metrics;",
    "metrics_24h": "select count(*) from metrics where timestamp > now() - interval '24 hours';",
    "logs_total": "select count(*) from logs;",
    "logs_24h": "select count(*) from logs where timestamp > now() - interval '24 hours';",
    "otlp_keys": "select count(*) from otlp_api_keys where enabled=true;",
    "otlp_dead_letter": "select count(*) from otlp_dead_letter;",
    "recent_services": "select coalesce(string_agg(service_name || ':' || c, ', '), '') from (select service_name, count(*) c from traces group by service_name order by c desc limit 5) s;",
    "recent_log_sources": "select coalesce(string_agg(coalesce(source,'null') || ':' || c, ', '), '') from (select source, count(*) c from logs group by source order by c desc limit 5) s;",
}

for name, sql in queries.items():
    try:
        print(f"DB {name} {psql(sql)}")
    except Exception as exc:
        print(f"DB {name} ERROR {exc}")
