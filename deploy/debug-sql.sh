#!/bin/bash
# Debug failing Oracle queries directly
ORACLE_USER="${ORACLE_USER:-SYS}"
ORACLE_PASSWORD="${ORACLE_PASSWORD:?Error: ORACLE_PASSWORD not set}"
ORACLE_HOST="${ORACLE_HOST:-10.88.218.59}"
ORACLE_PORT="${ORACLE_PORT:-1521}"
ORACLE_SERVICE="${ORACLE_SERVICE:-XEPDB1}"
SQLPLUS="LD_LIBRARY_PATH=/usr/lib/oracle/23/client64/lib /usr/lib/oracle/23/client64/bin/sqlplus -S '${ORACLE_USER}/${ORACLE_PASSWORD}@//${ORACLE_HOST}:${ORACLE_PORT}/${ORACLE_SERVICE} AS SYSDBA'"

run_sql() {
  local label="$1"
  local sql="$2"
  echo "=== $label ==="
  printf "SET PAGESIZE 0\nSET FEEDBACK OFF\nSET HEADING OFF\nSET LINESIZE 1000\n%s;\nEXIT;\n" "$sql" | LD_LIBRARY_PATH=/usr/lib/oracle/23/client64/lib /usr/lib/oracle/23/client64/bin/sqlplus -S "${ORACLE_USER}/${ORACLE_PASSWORD}@//${ORACLE_HOST}:${ORACLE_PORT}/${ORACLE_SERVICE} AS SYSDBA" 2>&1
  echo ""
}

run_sql "wait-classes" "SELECT wait_class || '|' || ROUND(time_waited_micro/1000,1) || '|' || total_waits || '|' || ROUND(CASE WHEN total_waits>0 THEN time_waited_micro/total_waits/1000 ELSE 0 END,2) FROM v\$system_wait_class WHERE wait_class != 'Idle' ORDER BY time_waited_micro DESC"

run_sql "top-queries" "SELECT sql_id || '|' || parsing_schema_name || '|' || executions || '|' || ROUND(elapsed_time/1000) || '|' || ROUND(cpu_time/1000) || '|' || buffer_gets || '|' || disk_reads || '|' || rows_processed || '|' || REPLACE(REPLACE(SUBSTR(sql_text,1,200), CHR(10), ' '), CHR(13), ' ') FROM v\$sqlstats WHERE executions > 0 ORDER BY elapsed_time DESC FETCH FIRST 5 ROWS ONLY"

run_sql "sysstat" "SELECT name || '|' || value FROM v\$sysstat WHERE name IN ('physical reads','physical writes','user commits','user rollbacks','parse count (total)','execute count','logons current','sorts (memory)','sorts (disk)')"

run_sql "sysmetrics" "SELECT metric_name || '|' || ROUND(value,2) FROM v\$sysmetric WHERE group_id=2"

run_sql "pdb" "SELECT p.pdb_id || '|' || p.pdb_name || '|' || p.status || '|' || p.open_mode || '|' || p.restricted FROM v\$pdbs p ORDER BY p.pdb_id"

run_sql "running-queries" "SELECT s.sid || '|' || s.serial# || '|' || NVL(s.username,'(background)') FROM v\$session s WHERE s.status='ACTIVE' AND s.type!='BACKGROUND' FETCH FIRST 5 ROWS ONLY"
