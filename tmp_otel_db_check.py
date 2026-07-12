import os
import subprocess


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
            "-c",
            sql,
        ],
        env={**os.environ, "PGPASSWORD": "zantaz"},
        text=True,
    )


queries = {
    "otlp_api_keys": "select id, key_prefix, enabled, expires_at, last_used_at, permissions from otlp_api_keys order by created_at desc;",
    "agent_api_keys": "select id, key_prefix, enabled, expires_at, revoked_at, last_used, permissions from agent_api_keys order by created_at desc limit 10;",
    "dead_letter_summary": "select signal, error, count(1) from otlp_dead_letter group by signal, error order by count(1) desc limit 10;",
    "dead_letter_recent": "select created_at, signal, error, left(payload::text, 200) as payload from otlp_dead_letter order by created_at desc limit 5;",
}

for name, sql in queries.items():
    print(f"--- {name} ---")
    try:
        print(psql(sql))
    except Exception as exc:
        print(f"ERROR {exc}")
