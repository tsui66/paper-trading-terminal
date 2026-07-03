#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

DB="data/test_cli.db"
CFG="data/test_mock_config.toml"
rm -f "$DB"

cargo build --quiet
PAPER="./target/debug/paper --config $CFG"

echo "== account =="
$PAPER --db "$DB" account --json | head -5

echo "== quote =="
$PAPER --db "$DB" quote AAPL MSFT --json | head -8

echo "== buy =="
$PAPER --db "$DB" buy AAPL --qty 10 --json

echo "== portfolio =="
$PAPER --db "$DB" portfolio --json | head -10

echo "== positions =="
$PAPER --db "$DB" positions --json

echo "== pnl =="
$PAPER --db "$DB" pnl --json

echo "== historical =="
$PAPER --db "$DB" historical AAPL --range m1 --interval d1 --json | head -8

echo "== history =="
$PAPER --db "$DB" history --json | head -8

echo "== limit buy =="
$PAPER --db "$DB" buy AAPL --qty 5 --limit 150 --json

echo "== orders =="
$PAPER --db "$DB" orders --json

ORDER_ID=$($PAPER --db "$DB" orders --json | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo "== cancel $ORDER_ID =="
$PAPER --db "$DB" cancel "${ORDER_ID:0:8}" --json

echo "== limit fill (aggressive buy) =="
$PAPER --db "$DB" buy MSFT --qty 2 --limit 9999 --json
$PAPER --db "$DB" orders --json

echo "== schema =="
$PAPER schema --json | head -10

echo "== config =="
$PAPER --db "$DB" config show --json

echo "== provider status =="
$PAPER --db "$DB" config provider-status --json | head -20

echo "All CLI tests passed."