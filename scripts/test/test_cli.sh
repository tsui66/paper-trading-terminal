#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
# shellcheck source=../lib/binary.sh
source "$ROOT/scripts/lib/binary.sh"

DB="data/test_cli.db"
CFG="data/test_mock_config.toml"
rm -f "$DB"

cargo build --quiet
PAPER_BIN="$(resolve_paper_bin debug)"
PAPER="$PAPER_BIN --config $CFG"

assert_contains() {
  local needle="$1"
  local haystack="$2"
  local label="${3:-output}"
  if ! echo "$haystack" | grep -q "$needle"; then
    echo "ASSERT FAILED [$label]: expected to contain: $needle" >&2
    echo "$haystack" >&2
    exit 1
  fi
}

echo "== account =="
OUT=$($PAPER --db "$DB" account --json)
assert_contains '"cash": 100000' "$OUT" "account.cash"
echo "$OUT" | head -5

echo "== quote =="
OUT=$($PAPER --db "$DB" quote AAPL MSFT --json)
assert_contains '"symbol": "AAPL"' "$OUT" "quote.AAPL"
assert_contains '"symbol": "MSFT"' "$OUT" "quote.MSFT"
echo "$OUT" | head -8

echo "== buy market =="
OUT=$($PAPER --db "$DB" buy AAPL --qty 10 --json)
assert_contains '"status": "filled"' "$OUT" "buy.filled"
assert_contains '"commission"' "$OUT" "buy.commission"
assert_contains '"avg_fill_price"' "$OUT" "buy.avg_fill_price"
echo "$OUT"

echo "== portfolio =="
OUT=$($PAPER --db "$DB" portfolio --json)
assert_contains '"positions": 1' "$OUT" "portfolio.positions"
echo "$OUT" | head -10

echo "== positions =="
OUT=$($PAPER --db "$DB" positions --json)
assert_contains '"locked_qty"' "$OUT" "positions.locked_qty"
assert_contains '"symbol": "AAPL"' "$OUT" "positions.AAPL"
echo "$OUT"

echo "== pnl =="
OUT=$($PAPER --db "$DB" pnl --json)
assert_contains '"initial_cash": 100000' "$OUT" "pnl.initial_cash"
echo "$OUT"

echo "== historical =="
OUT=$($PAPER --db "$DB" historical AAPL --range m1 --interval d1 --json)
assert_contains '"close"' "$OUT" "historical.close"
echo "$OUT" | head -8

echo "== history after buy =="
OUT=$($PAPER --db "$DB" history --json)
assert_contains '"side": "buy"' "$OUT" "history.buy"
echo "$OUT" | head -8

echo "== sell market =="
OUT=$($PAPER --db "$DB" sell AAPL --qty 5 --json)
assert_contains '"side": "sell"' "$OUT" "sell.side"
assert_contains '"status": "filled"' "$OUT" "sell.filled"
echo "$OUT"

echo "== positions after partial sell =="
OUT=$($PAPER --db "$DB" positions --json)
assert_contains '"quantity": 5' "$OUT" "positions.qty5"
echo "$OUT"

echo "== limit buy pending =="
OUT=$($PAPER --db "$DB" buy AAPL --qty 5 --limit 150 --json)
assert_contains '"status": "pending"' "$OUT" "limit.pending"
assert_contains '"order_type": "limit"' "$OUT" "limit.type"
echo "$OUT"

echo "== orders =="
OUT=$($PAPER --db "$DB" orders --json)
assert_contains '"status": "pending"' "$OUT" "orders.pending"
echo "$OUT"

ORDER_ID=$(echo "$OUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo "== cancel $ORDER_ID =="
OUT=$($PAPER --db "$DB" cancel "${ORDER_ID:0:8}" --json)
assert_contains '"status": "cancelled"' "$OUT" "cancel.status"
echo "$OUT"

echo "== limit fill (aggressive buy) =="
OUT=$($PAPER --db "$DB" buy MSFT --qty 2 --limit 9999 --json)
assert_contains '"status": "filled"' "$OUT" "aggressive.filled"
echo "$OUT"
OUT=$($PAPER --db "$DB" orders --json)
if [ "$OUT" != "[]" ]; then
  echo "ASSERT FAILED [orders.empty_after_fill]: expected [], got:" >&2
  echo "$OUT" >&2
  exit 1
fi
echo "$OUT"

echo "== limit sell pending + cancel =="
OUT=$($PAPER --db "$DB" sell MSFT --qty 1 --limit 9999 --json)
assert_contains '"side": "sell"' "$OUT" "limit_sell.side"
assert_contains '"status": "pending"' "$OUT" "limit_sell.pending"
SELL_ID=$(echo "$OUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
OUT=$($PAPER --db "$DB" cancel "${SELL_ID:0:8}" --json)
assert_contains '"status": "cancelled"' "$OUT" "limit_sell.cancel"
echo "$OUT"

echo "== upgrade --check =="
OUT=$($PAPER upgrade --check --json)
assert_contains '"current"' "$OUT" "upgrade.current"
assert_contains '"latest"' "$OUT" "upgrade.latest"
assert_contains '"update_available"' "$OUT" "upgrade.update_available"

echo "== schema =="
OUT=$($PAPER schema --json)
assert_contains '"cli_binary": "paper"' "$OUT" "schema.binary"
assert_contains '"buy"' "$OUT" "schema.buy"
assert_contains '"sell"' "$OUT" "schema.sell"
assert_contains '"upgrade"' "$OUT" "schema.upgrade"
echo "$OUT" | head -12

echo "== config show =="
OUT=$($PAPER --db "$DB" config show --json)
assert_contains '"provider": "mock"' "$OUT" "config.provider"
echo "$OUT"

echo "== config set-provider (temp file) =="
TMP_CFG=$(mktemp)
trap 'rm -f "$TMP_CFG"' EXIT
cp "$CFG" "$TMP_CFG"
OUT=$($PAPER_BIN --config "$TMP_CFG" --db "$DB" config set-provider mock --json)
assert_contains '"saved": true' "$OUT" "set-provider.saved"
assert_contains '"provider": "mock"' "$OUT" "set-provider.mock"

echo "== config set-fallback (temp file) =="
OUT=$($PAPER_BIN --config "$TMP_CFG" --db "$DB" config set-fallback mock --json)
assert_contains '"saved": true' "$OUT" "set-fallback.saved"
assert_contains '"fallback"' "$OUT" "set-fallback.list"

echo "== provider status =="
OUT=$($PAPER --db "$DB" config provider-status --json)
assert_contains '"status": "ok"' "$OUT" "provider.ok"
assert_contains '"provider": "mock"' "$OUT" "provider.mock"
echo "$OUT" | head -20

echo "All CLI tests passed."