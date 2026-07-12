import os
import subprocess


def psql(sql):
    return subprocess.check_output(
        ["psql", "-h", "127.0.0.1", "-p", "5434", "-U", "postgres", "-d", "zentra", "-c", sql],
        env={**os.environ, "PGPASSWORD": "zantaz"},
        text=True,
        stderr=subprocess.STDOUT,
    )


queries = {
    "otlp_api_keys": "select id, key_prefix, enabled, expires_at, last_used, permissions from otlp_api_keys order by created_at desc;",
    "agent_api_keys": "select id, display_prefix, enabled, expires_at, revoked_at, last_used from agent_api_keys order by created_at desc limit 10;",
    "dead_letter_summary": "select signal, error_message, count(1) from otlp_dead_letter group by signal, error_message order by count(1) desc limit 10;",
    "dead_letter_recent": "select created_at, signal, error_message, left(encode(payload, 'escape'), 200) as payload from otlp_dead_letter order by created_at desc limit 5;",
}

for name, sql in queries.items():
    print(f"--- {name} ---")
    print(psql(sql))
