#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
# shellcheck source=../lib/binary.sh
source "$ROOT/scripts/lib/binary.sh"

if ! command -v fcontext >/dev/null 2>&1; then
  echo "SKIP: Financial Context CLI not installed"
  exit 0
fi

cargo build --quiet
PAPER="$(resolve_paper_bin debug)"
DB="data/test_fcontext.db"
rm -f "$DB"

CFG="data/test_fcontext_config.toml"
cat > "$CFG" <<'EOF'
[provider]
default = "yahoo"
fallback = ["fcontext"]

[provider.fcontext]
cli = "fcontext"
timeout_secs = 30

[account]
initial_cash = 100000.0
currency = "USD"

[trading]
commission_per_trade = 0.0
slippage_bps = 5

[cache]
enabled = false
ttl_secs = 60

[watchlist]
symbols = ["AAPL"]
EOF

echo "== provider status =="
$PAPER --config "$CFG" --db "$DB" config provider-status || true

echo "== quote with fallback =="
$PAPER --config "$CFG" --db "$DB" quote AAPL --json | head -15

echo "== historical (fcontext only) =="
if $PAPER --config "$CFG" --db "$DB" historical AAPL --range m1 --interval d1 --json | head -10; then
  echo "historical OK"
else
  echo "SKIP: fcontext historical unavailable (e.g. subscription 402)"
fi

echo "fcontext integration test done."