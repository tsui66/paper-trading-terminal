# Agent install prompts

Copy a prompt below into **Claude Code**, **Codex**, or **OpenClaw** (or any terminal-capable coding agent). The agent should **run commands itself** and confirm each step — not only print instructions.

Repo: https://github.com/tsui66/paper-trading-terminal

---

## Universal (auto-detect OS)

```text
Install the paper-trading-terminal CLI (`paper`) on this machine.

Project: https://github.com/tsui66/paper-trading-terminal
Docs: README "Install & run" section in that repo.

Rules:
- Detect OS (macOS / Linux / Windows) and CPU arch yourself.
- Run install commands in the terminal; do not stop after a single failure — diagnose and retry.
- Prefer the official installer; build from source only if the installer fails or the user already has the repo cloned.

Install (pick one):
- macOS / Linux:
  curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
- Windows (PowerShell):
  iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

Alternatives if needed:
- macOS/Linux Homebrew: brew install --cask tsui66/tap/paper-trading-terminal
- Windows Scoop: scoop install https://github.com/tsui66/paper-trading-terminal/raw/refs/heads/main/.scoop/paper.json
- From source (Rust ≥ 1.91): git clone … && cargo build --release → ./target/release/paper

Verify (all must succeed):
1. paper -h
2. paper config provider-status   # yahoo should be ok; fcontext optional
3. paper quote AAPL               # expect price line with [yahoo] or [fcontext]
4. paper account

Optional smoke test:
- paper buy AAPL --qty 1 --json
- paper portfolio --json

Report: install path, `paper` version if shown, provider-status output, and quote result.
If `paper` is not on PATH after install, fix PATH or symlink and re-verify.

fcontext is optional fallback only — skip unless Yahoo fails or user asks.
```

---

## Claude Code

```text
You are in Claude Code with shell access. Install `paper` (paper-trading-terminal) for local US stock paper trading.

1. Detect platform. On macOS/Linux run:
   curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
   On Windows PowerShell run:
   iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

2. Verify: `paper -h`, `paper config provider-status`, `paper quote AAPL`, `paper account`.

3. If install fails, try Homebrew (macOS/Linux) or clone the repo and `cargo build --release`.

4. Summarize what worked. Do not mark done until `paper quote AAPL` returns a price.

Schema for later tool use: `paper schema --json`. Prefer `--json` on all commands for agents.
```

---

## Codex

```text
Task: install and verify paper-trading-terminal CLI (`paper`).

Source: https://github.com/tsui66/paper-trading-terminal

Execute:
- macOS/Linux: curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
- Windows: iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

Verification checklist:
[ ] paper -h prints help
[ ] paper config provider-status → yahoo ok
[ ] paper quote AAPL returns a quote
[ ] paper account shows cash/equity

On failure: read installer stderr, retry with brew/scoop or cargo build --release from a fresh clone.

Deliverable: short report with binary path, provider status, and one sample quote line.
Agent integration: use `paper <cmd> --json` and `paper schema --json` for command discovery.
```

---

## OpenClaw

```text
Goal: set up paper-trading-terminal so I can paper-trade US stocks from the terminal and via JSON CLI.

Install `paper`:
- Unix: curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
- Windows: iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

You must run these commands yourself in the sandbox/shell, not delegate to me.

After install:
- paper config provider-status
- paper quote AAPL
- paper account
- (optional) paper tui — only if interactive TUI is requested

If Yahoo is down, optionally install fcontext CLI (see README Step 4) and re-run provider-status.

For ongoing automation, register tools from `paper schema --json`. Example calls:
  paper portfolio --json
  paper buy AAPL --qty 10 --json
  paper sell MSFT --qty 5 --limit 500 --json

Confirm success only when quote and account commands work.
```

---

## Optional — fcontext fallback

Use when `paper config provider-status` shows Yahoo failing or the user wants a backup data source.

```text
Install optional fcontext CLI fallback for paper-trading-terminal.

paper already installed. Yahoo is primary; fcontext is fallback #2.

Steps:
1. Install fcontext:
   - macOS: brew install --cask aitaport/tap/fcontext-cli
   - Linux/macOS script: curl -sSL https://github.com/aitaport/fcontext-cli/releases/latest/download/install.sh | sh
   - Windows: iwr https://github.com/aitaport/fcontext-cli/releases/latest/download/install.ps1 | iex

2. Authenticate (interactive — user may need to complete browser login):
   fcontext auth login
   fcontext auth login --auth-code <CODE>
   fcontext auth status

3. Verify:
   fcontext quote AAPL.US --format json
   paper config provider-status   # fcontext should show ok
   paper quote AAPL

Report provider-status and whether quotes still work.
```

---

## Optional — post-install agent wiring

After `paper` is on PATH, use this to connect the agent to trading commands.

```text
Integrate paper-trading-terminal as a subprocess tool.

1. Run: paper schema --json — parse cli_commands and cli_global_flags.
2. Always append --json for machine-readable output.
3. Global flags: --config PATH (default ./config.toml), --db PATH (default data/paper.db).
4. Read-only: account, portfolio, positions, quote, historical, orders, history, pnl, config provider-status.
5. Mutations (confirm with user first): buy, sell, cancel.
6. Interactive TUI: paper tui — do not launch unless user asks.

Example flow:
  paper account --json
  paper quote NVDA AAPL --json
  paper portfolio --json

Never claim a trade executed without showing the JSON response from paper buy/sell.
```