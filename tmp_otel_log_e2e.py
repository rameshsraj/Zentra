import json
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

BASE = "http://127.0.0.1:5006"


def call(method, path, token=None, body=None):
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


def ns_now():
    return str(int(datetime.now(timezone.utc).timestamp() * 1_000_000_000))


status, body = call("POST", "/api/auth/login", body={"email": "admin@Zentra.local", "password": "changeme"})
payload = json.loads(body)
token = payload.get("access_token") or payload.get("token")
print("LOGIN", status, "token", bool(token))

status, body = call(
    "POST",
    "/api/otel/api-keys",
    token=token,
    body={"name": "codex-otel-log-test", "permissions": ["logs", "metrics", "traces", "profiles"]},
)
print("CREATE_KEY", status, body[:300].replace("\n", " "))
key_payload = json.loads(body)
raw_key = key_payload.get("raw_key") or key_payload.get("api_key") or key_payload.get("key")
print("KEY_PREFIX", raw_key[:13] if raw_key else None)

test_message = f"codex otel log e2e {int(time.time())}"
log_payload = {
    "resourceLogs": [
        {
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": "codex-otel-test"}},
                    {"key": "deployment.environment", "value": {"stringValue": "prodserver"}},
                ]
            },
            "scopeLogs": [
                {
                    "scope": {"name": "codex-check"},
                    "logRecords": [
                        {
                            "timeUnixNano": ns_now(),
                            "severityText": "INFO",
                            "body": {"stringValue": test_message},
                            "attributes": [
                                {"key": "test.case", "value": {"stringValue": "otel-log-e2e"}},
                            ],
                        }
                    ],
                }
            ],
        }
    ]
}

status, body = call("POST", "/v1/logs", token=raw_key, body=log_payload)
print("POST_LOG", status, body[:300].replace("\n", " "))
time.sleep(1.5)

status, body = call("GET", "/api/logs?limit=5", token=token)
print("API_LOGS", status, body[:800].replace("\n", " "))

status, body = call("GET", "/api/otel/ingest-audit?window=1h", token=token)
print("AUDIT", status, body[:800].replace("\n", " "))

print("TEST_MESSAGE", test_message)
