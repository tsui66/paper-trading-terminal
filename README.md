# paper-trading-terminal

**AI-native CLI for US stock paper trading** — with real-time market data, portfolio, and trading.

## Features

- **Paper account** — cash, positions, mark-to-market PnL, persisted in SQLite
- **Orders** — market and limit buy/sell, cancel, auto-fill when price crosses limit
- **Market data** — Yahoo (optional feature) with fcontext CLI fallback; fails loudly if both are down
- **TUI** — watchlist, Braille candlestick chart, in-app order entry, fill notifications
- **AI-native** — structured JSON I/O, `paper schema` for tool discovery, `AgentSkill` for Rust embeds
- **Rust library** — embed via `AgentSkill` and `TradingEngine`

## Requirements

- Rust stable (2024 edition)
- **fcontext** (optional) CLI on `PATH` when using the default provider chain (recommended for reliable quotes)
- **Yahoo provider** (optional): `cargo build --features yahoo` — requires Rust ≥ 1.91

## Installation

**Homebrew (macOS / Linux)** — requires a [Homebrew tap](.homebrew/Casks/paper-trading-terminal.rb) with release checksums:

```bash
brew install --cask <your-org>/tap/paper-trading-terminal
```

**Windows ([Scoop](https://scoop.sh))**

```powershell
scoop install https://github.com/tsui66/paper-trading-terminal/raw/refs/heads/main/.scoop/paper.json
```

**Windows (PowerShell)**

```powershell
iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex
```

**Install script (macOS / Linux)**

```bash
curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

Installs `paper` to `/usr/local/bin` (macOS/Linux) or `%LOCALAPPDATA%\Programs\paper` (Windows).

Override the GitHub repo for forks/self-hosted releases:

```bash
PAPER_INSTALL_REPO=your-org/paper-trading-terminal curl -sSL .../install | sh
```

### Build from source

```bash
git clone https://github.com/tsui66/paper-trading-terminal
cd paper-trading-terminal
cargo build --release
# binary: target/release/paper
make install-local   # optional: copy to /usr/local/bin
```

## fcontext CLI

`paper` uses the [Financial Context](https://docs.fcontext.com) CLI as the **fallback market-data provider** when Yahoo is unavailable. Install and sign in once; `paper` shells out to `fcontext` (or `fctx`) automatically.

### Install

**macOS (Homebrew)**

```bash
brew install --cask aitaport/tap/fcontext-cli
```

**Linux / macOS (script)**

```bash
curl -sSL https://github.com/aitaport/fcontext-cli/releases/latest/download/install.sh | sh
```

**Windows (Scoop)**

```powershell
scoop install https://github.com/aitaport/fcontext-cli/releases/latest/download/fcontext.json
```

**Windows (PowerShell)**

```powershell
iwr https://github.com/aitaport/fcontext-cli/releases/latest/download/install.ps1 | iex
```

Installers verify the release `.sha256` checksum and place both `fcontext` and `fctx` on your `PATH`.

### Authenticate

Sign in with OAuth (works in local terminals, SSH, and headless servers):

```bash
fcontext auth login
```

Open the printed URL in a browser, authorize, then redeem the code:

```bash
fcontext auth login --auth-code YOUR_CODE
```

The CLI stores the token locally (`oauth-token.json` in the fcontext config directory). Later commands reuse it automatically.

Check status or sign out:

```bash
fcontext auth status
fcontext auth status --format json
fcontext auth logout
```

### Verify

```bash
fcontext check
fcontext quote AAPL.US --format json
paper config provider-status
```

`paper` accepts symbols like `AAPL`; the fcontext provider maps them to `AAPL.US` internally.

More commands and agent-oriented JSON output: [fcontext CLI docs](https://docs.fcontext.com).

## Quick start

```bash
# Account & quotes
paper account
paper quote AAPL MSFT
paper portfolio --json

# Trade
paper buy AAPL --qty 10
paper buy MSFT --qty 5 --limit 500    # limit order (pending until price hits)
paper orders
paper cancel <order-id-prefix>
paper pnl

# Dashboard
paper tui
```

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
| 1 | **yahoo** | Build with `--features yahoo`; free, can be unstable |
| 2 | **fcontext** | Subprocess CLI; install + `fcontext auth login` (see [fcontext CLI](#fcontext-cli)) |
| — | **mock** | Synthetic prices; **dev/tests only**, never used as automatic fallback |

```
yahoo ──fail──► fcontext ──fail──► error (operation aborted)
```

Diagnose the chain:

```bash
paper config provider-status
```

Offline development (CI / no network):

```bash
paper config set-provider mock
```

## TUI

```bash
paper tui
```

![Paper Trading Terminal TUI](docs/tui-screenshot.png)

| Key | Action |
|-----|--------|
| `j` / `k` | Move watchlist selection |
| `b` / `s` | Buy / sell selected symbol |
| `m` | Toggle market / limit in order bar |
| `Tab` | Switch qty / limit field |
| `Enter` | Submit order |
| `Esc` | Cancel order entry |
| `n` | Select pending order |
| `x` | Cancel selected pending order |
| `r` | Refresh quotes and chart |
| `q` | Quit |

Limit fills ring the terminal bell and log `*** FILLED ***`.

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
| `config set-provider NAME` | `yahoo` \| `fcontext` \| `mock` |
| `config set-fallback a,b` | Comma-separated fallback list |
| `config provider-status` | Probe each provider + chain |
| `schema` | Agent integration schema (JSON) |
| `tui` | Launch dashboard |

**Ranges:** `d1` `d5` `m1` `m3` `m6` `y1` `y5`  
**Intervals:** `m1` `m5` `m15` `m30` `h1` `d1` `w1` `mo1`

## Configuration

`config.toml` at the project root (or path from `--config` / `PAPER_CONFIG`):

```toml
[account]
initial_cash = 100_000.0
currency = "USD"

[trading]
commission_per_trade = 0.0
slippage_bps = 5

[cache]
enabled = true
ttl_secs = 60

[watchlist]
symbols = ["AAPL", "MSFT", "NVDA", "GOOGL", "AMZN", "META", "TSLA"]
```

Copy `.env.example` for optional env overrides and `RUST_LOG`. fcontext auth is managed by the `fcontext` CLI, not `paper`.

## Agents & library

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
make test          # cargo test + mock CLI integration
make lint          # fmt + clippy
./scripts/test/test_fcontext.sh   # skips if fcontext CLI missing
```

Release packaging (local):

```bash
./scripts/package_release.sh                    # host tarball → dist/
./scripts/package_release.sh 0.1.0 darwin-arm64 linux-amd64 windows-amd64
PAPER_FEATURES=yahoo ./scripts/package_release.sh   # Rust >= 1.91
```

Push a `v*` tag to trigger [`.github/workflows/release.yml`](.github/workflows/release.yml) (multi-platform GitHub Release).

### Project layout

```
src/
  cli/          # Clap commands
  engine/       # TradingEngine, orders, fills
  provider/     # yahoo, fcontext, mock, fallback chain
  tui/          # Ratatui dashboard
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