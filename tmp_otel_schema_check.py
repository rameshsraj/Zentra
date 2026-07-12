import os
import subprocess

env = {**os.environ, "PGPASSWORD": "zantaz"}
base = ["psql", "-h", "127.0.0.1", "-p", "5434", "-U", "postgres", "-d", "zentra"]
for table in ["otlp_api_keys", "agent_api_keys", "otlp_dead_letter", "logs", "traces", "metrics"]:
    print(f"--- {table} columns ---")
    sql = (
        "select column_name, data_type from information_schema.columns "
        f"where table_schema='public' and table_name='{table}' order by ordinal_position;"
    )
    print(subprocess.check_output(base + ["-Atc", sql], env=env, text=True))
