<h1 align="center">paper-trading-terminal</h1>

<p align="center"><strong>AI-native CLI for US stock paper trading</strong> — with real-time market data, portfolio, and trading.</p>

<p align="center"><strong>Languages:</strong> English · <a href="README.zh-CN.md">简体中文</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></p>

## Features

- **Paper account** — cash, positions, mark-to-market PnL, persisted in SQLite; TUI reset (`z`) restores `initial_cash` and clears positions/orders
- **Orders** — market and limit buy/sell, cancel, auto-fill when price crosses limit; pending orders show fill price and fees in the TUI
- **Realistic simulation** — session-aware execution (regular / pre / after-hours / closed), lot sizes, A-share price bands & T+1 sell lock, per-market regulatory fees + configurable broker commission
- **Market data** — Yahoo first, fcontext CLI fallback; fails loudly if both are down
- **TUI** — adaptive layout (resizes panels & compact text on smaller terminals), watchlist, Braille candlestick chart with page scroll, in-app order entry, fill notifications
- **AI-native** — structured JSON I/O, `paper schema` for tool discovery, `AgentSkill` for Rust embeds
- **Rust library** — embed via `AgentSkill` and `TradingEngine`

## Requirements

- **paper** binary on `PATH` (see [Install & run](#install--run) below)
- **fcontext** CLI — *optional*; used as fallback when Yahoo is unavailable

Build from source additionally needs Rust stable ≥ 1.91 (Yahoo enabled by default).

### AI agent install (Claude / Codex / OpenClaw)

Prefer letting a coding agent install and verify for you. Copy a prompt from **[docs/agent-install.md](docs/agent-install.md)** into Claude Code, Codex, or OpenClaw — the agent should run the installer and confirm `paper quote AAPL` works.

<details>
<summary>Quick copy — universal install prompt</summary>

```text
Install the paper-trading-terminal CLI (`paper`) on this machine.

Project: https://github.com/tsui66/paper-trading-terminal

Rules:
- Detect OS yourself and run the official installer (do not only print commands).
- macOS/Linux: curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
- Windows PowerShell: iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

Verify: paper -h → paper config provider-status → paper quote AAPL → paper account.
Retry with Homebrew / Scoop / cargo build --release if the script fails.
Report install path and verification output. fcontext is optional unless Yahoo fails.
```

More prompts (per-agent wording, fcontext fallback, JSON tool wiring): [docs/agent-install.md](docs/agent-install.md).

</details>

## Install & run

Follow the steps in order. Each step prints hints — match them to confirm success.

### Step 1 — Install `paper`

Pick your platform and run **one** command.

**macOS / Linux (recommended)**

```bash
curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
export PATH="$HOME/.local/bin:$PATH"   # if `paper` not found
```

Installs to `~/.local/bin` by default — **no sudo / password** (agent-friendly). System-wide: `PAPER_INSTALL_SYSTEM=1` before the curl pipe.

You should see:

```text
Installing paper-trading-terminal@v…
Downloading https://github.com/tsui66/paper-trading-terminal/releases/download/…
paper CLI v… installed to /Users/you/.local/bin/paper

Next steps:
  paper -h                      # verify install
  paper config provider-status  # check yahoo + fcontext (optional)
  paper quote AAPL              # test live quote
  paper tui                     # launch dashboard
```

**Windows (PowerShell)**

```powershell
iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex
```

You should see `paper CLI v… installed` and (first time) `Added …\Programs\paper to your PATH`. **Restart the terminal** if `paper` is not found.

<details>
<summary>Other install methods</summary>

**Homebrew (macOS / Linux)**

```bash
brew install --cask tsui66/tap/paper-trading-terminal
```

**Windows ([Scoop](https://scoop.sh))**

```powershell
scoop install https://github.com/tsui66/paper-trading-terminal/raw/refs/heads/main/.scoop/paper.json
```

**Build from source** (needs Rust ≥ 1.91)

```bash
git clone https://github.com/tsui66/paper-trading-terminal
cd paper-trading-terminal
cargo build --release
# binary: ./target/release/paper
make install-local   # optional: copy to ~/.local/bin (no sudo)
```

Fork or self-hosted releases:

```bash
PAPER_INSTALL_REPO=your-org/paper-trading-terminal curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

</details>

### Upgrade

```bash
paper upgrade --check          # compare with latest GitHub release
paper upgrade                  # download and replace current binary
paper upgrade --version v0.0.2 # install a specific version
```

Uses [GitHub Releases](https://github.com/tsui66/paper-trading-terminal/releases). Override repo with `PAPER_INSTALL_REPO=owner/name` or `--repo`.

### Step 2 — Verify `paper`

```bash
paper -h
paper config provider-status
```

Expected: help text prints; `yahoo` shows **ok** (primary). `fcontext` may show **missing** until Step 4 — that is fine for a first run.

```bash
paper quote AAPL
```

Expected: a line with price and change, e.g. `AAPL  $…  +….%  [yahoo]`.

### Step 3 — Run

```bash
paper account          # cash & equity
paper tui              # interactive dashboard
```

In the TUI: `j`/`k` move the watchlist, `b`/`s` buy/sell, `Tab` switch chart period, `←`/`→` flip chart pages (older / newer), `z` reset account (double-confirm), `q` quit.

**Paper trading (CLI)**

```bash
paper buy AAPL --qty 10
paper buy MSFT --qty 5 --limit 500   # limit order — pending until price hits
paper orders
paper cancel <order-id-prefix>
paper portfolio --json
```

### Step 4 — (Optional) Install fcontext fallback

Skip this if Yahoo quotes already work. Install fcontext when:

- `paper config provider-status` shows `fcontext: missing` and you want a backup source
- Yahoo is flaky and quotes fail intermittently
- You need symbols or data Yahoo does not cover

`paper` shells out to `fcontext` (or `fctx`) automatically — no extra config beyond `config.toml` defaults.

**4a. Install the CLI**

macOS (Homebrew):

```bash
brew install --cask aitaport/tap/fcontext-cli
```

Linux / macOS (script):

```bash
curl -sSL https://github.com/aitaport/fcontext-cli/releases/latest/download/install.sh | sh
```

Windows (PowerShell):

```powershell
iwr https://github.com/aitaport/fcontext-cli/releases/latest/download/install.ps1 | iex
```

Windows (Scoop):

```powershell
scoop install https://github.com/aitaport/fcontext-cli/releases/latest/download/fcontext.json
```

You should have `fcontext` (and `fctx`) on `PATH`:

```bash
fcontext -h
```

**4b. Sign in (one time)**

```bash
fcontext auth login
```

Open the URL in a browser, authorize, then:

```bash
fcontext auth login --auth-code YOUR_CODE
```

Check:

```bash
fcontext auth status
fcontext check
```

**4c. Verify with `paper`**

```bash
fcontext quote AAPL.US --format json
paper config provider-status
paper quote AAPL
```

Expected: `fcontext` shows **ok** in provider-status; quotes may show `[yahoo]` or `[fcontext]` depending on which provider answered.

`paper` accepts `AAPL`; fcontext uses `AAPL.US` internally.

More: [fcontext CLI docs](https://docs.fcontext.com).

## Quick reference

Global flags (all commands):

| Flag | Description |
|------|-------------|
| `--json` | Machine-readable output |
| `--config PATH` | Config file (default: `./config.toml`) |
| `--db PATH` | SQLite path (default: `data/paper.db`) |

Environment: `PAPER_CONFIG` overrides the config path (legacy: `PPT_CONFIG`).

## Market data providers

Default chain in `config.toml`:

```toml
[provider]
default = "yahoo"
fallback = ["fcontext"]

[provider.fcontext]
cli = "fcontext"
timeout_secs = 30
```

| Priority | Provider | Notes |
|----------|----------|-------|
| 1 | **yahoo** | Default; free Yahoo Finance data (can be unstable) |
| 2 | **fcontext** | Fallback CLI; install + `fcontext auth login` (see [fcontext CLI](#fcontext-cli)) |

```
yahoo ──fail──► fcontext ──fail──► error (operation aborted)
```

Diagnose the chain:

```bash
paper config provider-status
```

## TUI

```bash
paper tui
```

![Paper Trading Terminal TUI](docs/tui-screenshot.png)

The dashboard adapts to terminal size: panel widths, row heights, and table text scale automatically on smaller windows (recommended minimum ~80×24). Shortcut hints in the footer shrink in compact mode.

| Key | Action |
|-----|--------|
| `j` / `k` or `↓` / `↑` | Move watchlist selection |
| `Enter` | Select highlighted watchlist symbol (loads chart) |
| `Tab` / `Shift-Tab` | Chart period (1m … Year); resets chart to latest page |
| `←` / `→` | Chart pages — **one key press = one full screen** of bars (← older, → newer) |
| `b` / `s` | Buy / sell selected symbol |
| `m` | Toggle market / limit in order bar |
| `Enter` | Submit order (when order bar is active) |
| `Esc` | Cancel order entry, or cancel account-reset confirm |
| `n` | Cycle selected pending order |
| `x` | Cancel selected pending order |
| `z` | Reset account — press twice to confirm; restores `initial_cash`, clears positions & orders |
| `r` | Refresh quotes and chart |
| `q` | Quit |

**Panels:** watchlist (left), candlestick chart (center), holdings + pending orders (right), log, order/shortcut bar (bottom). Orders table columns include symbol, side, type, qty, fill price, fee, and status; a detail line shows the selected order.

**Chart navigation:** page 0 shows the most recent bars. `←` loads the previous (older) page; older history is fetched on demand. `→` returns toward the latest page. Arrow keys are ignored while the order entry bar is open (`Esc` to exit).

Limit fills ring the terminal bell and log `*** FILLED ***` with fee breakdown.

## CLI reference

| Command | Description |
|---------|-------------|
| `account` | Cash, equity, open position count |
| `portfolio` | Mark-to-market breakdown |
| `positions` | Open holdings |
| `quote SYM [SYM…]` | Live quotes |
| `historical SYM --range m6 --interval d1` | OHLCV candles |
| `buy SYM --qty N [--limit P]` | Market or limit buy |
| `sell SYM --qty N [--limit P]` | Market or limit sell |
| `orders` | Pending limit orders |
| `cancel ID` | Cancel by UUID or unique prefix |
| `history` | Filled / cancelled order log |
| `pnl` | Realized + unrealized P&L |
| `config show` | Current settings |
| `config set-provider NAME` | `yahoo` \| `fcontext` |
| `config set-fallback a,b` | Comma-separated fallback list |
| `config provider-status` | Probe each provider + chain |
| `schema` | Agent integration schema (JSON) |
| `upgrade` | Download latest release and replace the `paper` binary |
| `upgrade --check` | Check if a newer release is available |
| `upgrade --version v0.0.2` | Install a specific release tag |
| `tui` | Launch dashboard |

**Ranges:** `d1` `d5` `m1` `m3` `m6` `y1` `y5`  
**Intervals:** `m1` `m5` `m15` `m30` `h1` `d1` `w1` `mo1`

## Trading simulation

Paper fills respect market rules derived from the symbol suffix and live quote session status:

| Rule | US | HK | A-share (`.SH` / `.SZ`) |
|------|----|----|-------------------------|
| Lot size | 1 share | 100 shares (default) | 100 shares |
| T+1 sell lock | No | No | Yes — bought shares locked until next session |
| Extended hours | Pre/after market orders allowed | Limit queue when closed | Follows CN session |
| Price bands | — | — | ±10% (±5% for ST names) on limit orders |
| Regulatory fees | SEC / FINRA on sells | Stamp duty, levies | Stamp duty, transfer fee |

Platform commission is configurable; regulatory fees are always modeled:

```toml
[trading]
commission_per_trade = 0.0   # flat per order
commission_bps = 0.0         # notional bps (1 bps = 0.01%)
min_commission = 0.0
slippage_bps = 5.0
```

When the exchange is **closed**, market orders are rejected; limit orders may queue. **Halted** or **suspended** symbols reject all orders.

## Configuration

`config.toml` at the project root (or path from `--config` / `PAPER_CONFIG`):

```toml
[account]
initial_cash = 100_000.0
currency = "USD"

[trading]
commission_per_trade = 0.0
commission_bps = 0.0
min_commission = 0.0
slippage_bps = 5.0

[cache]
enabled = true
ttl_secs = 60

[watchlist]
symbols = ["AAPL", "MSFT", "NVDA", "GOOGL", "AMZN", "META", "TSLA"]
```

Copy `.env.example` for optional env overrides and `RUST_LOG`. fcontext auth is managed by the `fcontext` CLI, not `paper`.

Account reset is available in the TUI (`z`, double-confirm) only — there is no CLI `reset` command yet.

## Agents & library

**Install via agent:** [docs/agent-install.md](docs/agent-install.md) — copy-paste prompts for Claude Code, Codex, and OpenClaw.

Discover the CLI contract:

```bash
paper schema --json
```

Example subprocess integration:

```bash
paper portfolio --json
paper buy AAPL --qty 10 --json
```

Rust embedding:

```rust
use paper_trading_terminal::cli::AppState;
use paper_trading_terminal::skill::{agent_schema, AgentSkill};
use paper_trading_terminal::{create_provider_stack, AppConfig, QuoteCache};

let config = AppConfig::load(None)?;
let provider = create_provider_stack(&config, Some(QuoteCache::new(true, 60)));
let skill = AgentSkill::new(AppState::new(config, provider));
let _schema = agent_schema();
```

## Development

```bash
make test          # cargo test + CLI integration
make lint          # fmt + clippy
./scripts/test/test_fcontext.sh   # skips if fcontext CLI missing
```

Release packaging (local):

```bash
./scripts/package_release.sh                    # host tarball → dist/
./scripts/package_release.sh 0.1.0 darwin-arm64 linux-amd64 windows-amd64
cargo build --no-default-features   # slim binary without Yahoo
```

Push a `v*` tag to trigger [`.github/workflows/release.yml`](.github/workflows/release.yml) (multi-platform GitHub Release).

### Project layout

```
src/
  cli/          # Clap commands
  engine/       # TradingEngine, orders, fills, market_rules, tradability
  provider/     # yahoo, fcontext, fallback chain
  tui/          # Ratatui dashboard (ui/layout adaptive sizing, kline pagination)
  skill.rs      # AgentSkill + schema
data/           # SQLite DBs (gitignored), test configs
scripts/
  build_release.sh
  test/         # Shell integration tests
```

### Architecture

```
┌─────────┐   ┌─────────┐
│   CLI   │   │   TUI   │
└────┬────┘   └────┬────┘
     │             │
     └──────┬──────┘
            ▼
     TradingEngine ──► SQLite (account, orders, positions)
            │
            ▼
   FallbackProvider (yahoo → fcontext)
```

## Disclaimer

**For research and learning only.** This project is a **simulated paper-trading** tool. It does not connect to a brokerage, execute real orders, or provide investment advice. Market data may be delayed or inaccurate. You are solely responsible for how you use this software.

**免责声明：** 本工具仅供模拟盘**研究与学习**使用，不构成任何投资建议，不涉及真实股票交易。

## License

MIT — see [LICENSE](LICENSE).