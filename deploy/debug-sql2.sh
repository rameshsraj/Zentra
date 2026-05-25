#!/bin/bash
ORACLE_USER="${ORACLE_USER:-SYS}"
ORACLE_PASSWORD="${ORACLE_PASSWORD:?Error: ORACLE_PASSWORD not set}"
ORACLE_HOST="${ORACLE_HOST:-10.88.218.59}"
ORACLE_PORT="${ORACLE_PORT:-1521}"
ORACLE_SERVICE="${ORACLE_SERVICE:-XEPDB1}"

run_sql() {
  local label="$1"
  local sql="$2"
  echo "=== $label ==="
  printf "SET PAGESIZE 0\nSET FEEDBACK OFF\nSET HEADING OFF\nSET LINESIZE 1000\n%s;\nEXIT;\n" "$sql" | LD_LIBRARY_PATH=/usr/lib/oracle/23/client64/lib /usr/lib/oracle/23/client64/bin/sqlplus -S "${ORACLE_USER}/${ORACLE_PASSWORD}@//${ORACLE_HOST}:${ORACLE_PORT}/${ORACLE_SERVICE} AS SYSDBA" 2>&1
  echo ""
}

# Check v$sqlstats columns
run_sql "sqlstats-cols" "SELECT column_name FROM all_tab_columns WHERE table_name='V_\$SQLSTATS' ORDER BY column_id"

# Check v$sqlarea columns (alternative)  
run_sql "sqlarea-cols" "SELECT column_name FROM all_tab_columns WHERE table_name='V_\$SQLAREA' ORDER BY column_id FETCH FIRST 20 ROWS ONLY"

# Check v$pdbs columns
run_sql "pdbs-cols" "SELECT column_name FROM all_tab_columns WHERE table_name='V_\$PDBS' ORDER BY column_id"

# Check v$sysmetric - try different group_id
run_sql "sysmetric-groups" "SELECT DISTINCT group_id, COUNT(*) FROM v\$sysmetric GROUP BY group_id"

# Try sysmetric without group filter
run_sql "sysmetric-sample" "SELECT metric_name || '|' || ROUND(value,2) FROM v\$sysmetric FETCH FIRST 5 ROWS ONLY"

# Check listeners view
run_sql "listener-net" "SELECT column_name FROM all_tab_columns WHERE table_name='V_\$LISTENER_NETWORK' ORDER BY column_id"

# rman view
run_sql "rman-cols" "SELECT column_name FROM all_tab_columns WHERE table_name='V_\$RMAN_BACKUP_JOB_DETAILS' ORDER BY column_id FETCH FIRST 10 ROWS ONLY"

# perf history
run_sql "perfhist-sample" "SELECT metric_name FROM v\$sysmetric_history WHERE ROWNUM=1"
