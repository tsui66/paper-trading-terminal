<h1 align="center">paper-trading-terminal</h1>

<p align="center"><strong>面向 AI 的美股模拟盘 CLI</strong> — 实时行情、持仓组合与交易。</p>

<p align="center"> <a href="README.md">English</a> · 简体中文 · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></p>

## 功能

- **模拟账户** — 现金、持仓、按市值盈亏，持久化于 SQLite；TUI 中 `z` 可重置为 `initial_cash` 并清空持仓与订单
- **订单** — 市价/限价买卖、撤单，触及限价自动成交；TUI 挂单展示成交价与费用
- **贴近真实的模拟** — 按交易时段执行（盘中/盘前盘后/休市）、整手规则、A 股涨跌停与 T+1 卖出锁定、各市场监管费 + 可配置券商佣金
- **行情** — 优先 Yahoo，失败时回退 Financial Context CLI；两者均不可用时会明确报错
- **TUI** — 自适应布局（小终端自动缩面板与字号）、自选股、Braille K 线整页翻动、内置下单、成交提醒
- **AI 原生** — 结构化 JSON 输出、`paper schema` 工具发现、`AgentSkill` Rust 嵌入
- **Rust 库** — 通过 `AgentSkill` 与 `TradingEngine` 嵌入应用

## 环境要求

- **paper** 二进制在 `PATH` 中（见下方 [安装与运行](#安装与运行)）
- **Financial Context** CLI — *可选*；Yahoo 不可用时作为备用数据源

从源码构建另需 Rust stable ≥ 1.91（默认启用 Yahoo）。

### AI 代理安装（Claude / Codex / OpenClaw）

可将安装交给编程代理完成。把 **[docs/agent-install.md](docs/agent-install.md)** 中的提示词复制到 Claude Code、Codex 或 OpenClaw，由代理执行安装并确认 `paper quote AAPL` 可用。

<details>
<summary>快速复制 — 通用安装提示词</summary>

```text
在本机安装 paper-trading-terminal CLI（`paper`）。

项目：https://github.com/tsui66/paper-trading-terminal

要求：
- 自行识别操作系统并执行官方安装脚本（不要只打印命令）。
- macOS/Linux：curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
- Windows PowerShell：iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

验证：paper -h → paper config provider-status → paper quote AAPL → paper account。
脚本失败时改用 Homebrew / Scoop / cargo build --release。
汇报安装路径与验证输出。除非 Yahoo 失败，否则 fcontext 为可选。
```

更多提示词（分代理、Financial Context CLI 备用、JSON 工具接入）：[docs/agent-install.md](docs/agent-install.md)。

</details>

## 安装与运行

按顺序执行。每步终端会有提示，对照确认是否成功。

### 步骤 1 — 安装 `paper`

按平台选择 **一条** 命令。

**macOS / Linux（推荐）**

```bash
curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

预期输出类似：

```text
Installing paper-trading-terminal@v…
Downloading https://github.com/tsui66/paper-trading-terminal/releases/download/…
paper CLI v… installed to /usr/local/bin/paper

Next steps:
  paper -h                      # verify install
  paper config provider-status  # check yahoo + fcontext (optional)
  paper quote AAPL              # test live quote
  paper tui                     # launch dashboard
```

**Windows（PowerShell）**

```powershell
iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex
```

应看到 `paper CLI v… installed`；首次可能提示已加入 PATH。**若找不到 `paper`，请重启终端。**

<details>
<summary>其他安装方式</summary>

**Homebrew（macOS / Linux）**

```bash
brew install --cask tsui66/tap/paper-trading-terminal
```

**Windows（[Scoop](https://scoop.sh)）**

```powershell
scoop install https://github.com/tsui66/paper-trading-terminal/raw/refs/heads/main/.scoop/paper.json
```

**从源码构建**（需 Rust ≥ 1.91）

```bash
git clone https://github.com/tsui66/paper-trading-terminal
cd paper-trading-terminal
cargo build --release
# 二进制：./target/release/paper
make install-local   # 可选：复制到 /usr/local/bin
```

Fork 或自建 Release：

```bash
PAPER_INSTALL_REPO=your-org/paper-trading-terminal curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

</details>

### 升级

```bash
paper upgrade --check          # 与 GitHub 最新 release 对比
paper upgrade                  # 下载并替换当前二进制
paper upgrade --version v0.0.2 # 安装指定版本
```

使用 [GitHub Releases](https://github.com/tsui66/paper-trading-terminal/releases)。可用 `PAPER_INSTALL_REPO=owner/name` 或 `--repo` 覆盖仓库。

### 步骤 2 — 验证 `paper`

```bash
paper -h
paper config provider-status
```

预期：打印帮助；`yahoo` 为 **ok**（主源）。首次运行 `fcontext` 可能为 **missing**，可先做步骤 4。

```bash
paper quote AAPL
```

预期：一行报价，例如 `AAPL  $…  +….%  [yahoo]`。

### 步骤 3 — 运行

```bash
paper account          # 现金与权益
paper tui              # 交互式仪表盘
```

TUI 快捷键：`j`/`k` 移动自选、`b`/`s` 买卖、`Tab` 切换 K 线周期、`←`/`→` 整页翻动 K 线（更早/更新）、`z` 双确认重置账户、`q` 退出。

**命令行模拟交易**

```bash
paper buy AAPL --qty 10
paper buy MSFT --qty 5 --limit 500   # 限价单 — 触及价格后成交
paper orders
paper cancel <order-id-prefix>
paper portfolio --json
```

### 步骤 4 —（可选）安装 Financial Context CLI 备用源

若 Yahoo 已正常可跳过。建议在以下情况安装：

- `paper config provider-status` 显示 `fcontext: missing` 且需要备用源
- Yahoo 不稳定、报价间歇失败
- 需要 Yahoo 未覆盖的标的或数据

`paper` 会自动调用 `fcontext`（或 `fctx`），默认 `config.toml` 即可，无需额外配置。

**4a. 安装 CLI**

macOS（Homebrew）：

```bash
brew install --cask aitaport/tap/fcontext-cli
```

Linux / macOS（脚本）：

```bash
curl -sSL https://github.com/aitaport/fcontext-cli/releases/latest/download/install.sh | sh
```

Windows（PowerShell）：

```powershell
iwr https://github.com/aitaport/fcontext-cli/releases/latest/download/install.ps1 | iex
```

Windows（Scoop）：

```powershell
scoop install https://github.com/aitaport/fcontext-cli/releases/latest/download/fcontext.json
```

确认 `fcontext`（及 `fctx`）在 `PATH`：

```bash
fcontext -h
```

**4b. 登录（一次性）**

```bash
fcontext auth login
```

浏览器打开 URL 授权后：

```bash
fcontext auth login --auth-code YOUR_CODE
```

检查：

```bash
fcontext auth status
fcontext check
```

**4c. 用 `paper` 验证**

```bash
fcontext quote AAPL.US --format json
paper config provider-status
paper quote AAPL
```

预期：provider-status 中 `fcontext` 为 **ok**；报价可能显示 `[yahoo]` 或 `[fcontext]`。

`paper` 接受 `AAPL`；fcontext 内部使用 `AAPL.US`。

更多：[Financial Context CLI 文档](https://docs.fcontext.com)。

## 速查

全局参数（所有命令）：

| 参数 | 说明 |
|------|------|
| `--json` | 机器可读 JSON 输出 |
| `--config PATH` | 配置文件（默认 `./config.toml`） |
| `--db PATH` | SQLite 路径（默认 `data/paper.db`） |

环境变量：`PAPER_CONFIG` 覆盖配置路径（旧名：`PPT_CONFIG`）。

## 行情数据源

`config.toml` 默认链路：

```toml
[provider]
default = "yahoo"
fallback = ["fcontext"]

[provider.fcontext]
cli = "fcontext"
timeout_secs = 30
```

| 优先级 | 提供商 | 说明 |
|--------|--------|------|
| 1 | **yahoo** | 默认；免费 Yahoo Finance（可能不稳定） |
| 2 | **fcontext** | Financial Context CLI 备用；需安装并 `fcontext auth login` |

```
yahoo ──失败──► fcontext ──失败──► 报错（中止操作）
```

诊断：

```bash
paper config provider-status
```

## TUI

```bash
paper tui
```

![Paper Trading Terminal TUI](docs/tui-screenshot.png)

仪表盘随终端尺寸自适应：面板宽度、行高、表格字号在小窗口自动紧凑（建议最小约 80×24）。底部快捷键在紧凑模式下会缩短。

| 按键 | 操作 |
|------|------|
| `j` / `k` 或 `↓` / `↑` | 移动自选高亮 |
| `Enter` | 选中自选标的（加载图表） |
| `Tab` / `Shift-Tab` | K 线周期（1m … Year）；重置到最新页 |
| `←` / `→` | K 线翻页 — **按一次键 = 一整屏 K 线**（← 更早，→ 更新） |
| `b` / `s` | 买入 / 卖出当前标的 |
| `m` | 下单栏切换市价 / 限价 |
| `Enter` | 提交订单（下单栏激活时） |
| `Esc` | 取消下单，或取消账户重置确认 |
| `n` | 切换选中的挂单 |
| `x` | 撤销选中的挂单 |
| `z` | 重置账户 — 连按两次确认；恢复 `initial_cash`，清空持仓与订单 |
| `r` | 刷新报价与图表 |
| `q` | 退出 |

**面板：** 自选（左）、K 线（中）、持仓 + 挂单（右）、日志、下单/快捷键栏（底）。订单表含代码、方向、类型、数量、成交价、费用、状态；选中行有详情。

**图表导航：** 第 0 页为最新 K 线。`←` 加载更早一页，按需拉取历史。`→` 回到较新页面。下单栏打开时方向键无效（`Esc` 退出）。

限价成交会响铃并在日志输出 `*** FILLED ***` 及费用明细。

## CLI 参考

| 命令 | 说明 |
|------|------|
| `account` | 现金、权益、持仓数 |
| `portfolio` | 按市值组合明细 |
| `positions` | 当前持仓 |
| `quote SYM [SYM…]` | 实时报价 |
| `historical SYM --range m6 --interval d1` | OHLCV K 线 |
| `buy SYM --qty N [--limit P]` | 市价或限价买入 |
| `sell SYM --qty N [--limit P]` | 市价或限价卖出 |
| `orders` | 待成交限价单 |
| `cancel ID` | 按 UUID 或前缀撤单 |
| `history` | 已成交 / 已撤销记录 |
| `pnl` | 已实现 + 未实现盈亏 |
| `config show` | 当前配置 |
| `config set-provider NAME` | `yahoo` \| `fcontext` |
| `config set-fallback a,b` | 逗号分隔备用列表 |
| `config provider-status` | 探测各数据源与链路 |
| `schema` | Agent 集成 schema（JSON） |
| `upgrade` | 下载最新 release 并替换 `paper` 二进制 |
| `upgrade --check` | 检查是否有新版本 |
| `upgrade --version v0.0.2` | 安装指定 release 标签 |
| `tui` | 启动仪表盘 |

**区间：** `d1` `d5` `m1` `m3` `m6` `y1` `y5`  
**周期：** `m1` `m5` `m15` `m30` `h1` `d1` `w1` `mo1`

## 交易模拟规则

成交遵循标的后缀与实时行情中的交易时段：

| 规则 | 美股 | 港股 | A 股（`.SH` / `.SZ`） |
|------|------|------|------------------------|
| 整手 | 1 股 | 100 股（默认） | 100 股 |
| T+1 卖出锁定 | 否 | 否 | 是 — 当日买入次日才可卖 |
| 延长交易 | 允许盘前盘后市价 | 休市时限价可排队 | 按 A 股时段 |
| 涨跌停 | — | — | 限价 ±10%（ST ±5%） |
| 监管费用 | 卖出 SEC / FINRA | 印花税、征费 | 印花税、过户费 |

平台佣金可配置；监管费始终计入：

```toml
[trading]
commission_per_trade = 0.0   # 每笔固定
commission_bps = 0.0         # 成交额 bps（1 bps = 0.01%）
min_commission = 0.0
slippage_bps = 5.0
```

**休市** 时拒绝市价单；限价单可排队。**停牌** 或 **暂停上市** 标的拒绝一切订单。

## 配置

项目根目录 `config.toml`（或 `--config` / `PAPER_CONFIG` 指定路径）：

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

可选环境变量见 `.env.example` 与 `RUST_LOG`。Financial Context 认证由 Financial Context CLI 管理，非 `paper`。

账户重置仅在 TUI（`z` 双确认）— 尚无 CLI `reset` 命令。

## Agent 与库

**通过 Agent 安装：** [docs/agent-install.md](docs/agent-install.md) — Claude Code、Codex、OpenClaw 可复制提示词。

发现 CLI 契约：

```bash
paper schema --json
```

子进程集成示例：

```bash
paper portfolio --json
paper buy AAPL --qty 10 --json
```

Rust 嵌入：

```rust
use paper_trading_terminal::cli::AppState;
use paper_trading_terminal::skill::{agent_schema, AgentSkill};
use paper_trading_terminal::{create_provider_stack, AppConfig, QuoteCache};

let config = AppConfig::load(None)?;
let provider = create_provider_stack(&config, Some(QuoteCache::new(true, 60)));
let skill = AgentSkill::new(AppState::new(config, provider));
let _schema = agent_schema();
```

## 开发

```bash
make test          # cargo test + CLI 集成测试
make lint          # fmt + clippy
./scripts/test/test_fcontext.sh   # 无 fcontext 时跳过
```

本地打包：

```bash
./scripts/package_release.sh                    # 本机 tarball → dist/
./scripts/package_release.sh 0.1.0 darwin-arm64 linux-amd64 windows-amd64
cargo build --no-default-features   # 无 Yahoo 的精简版
```

推送 `v*` 标签触发 [`.github/workflows/release.yml`](.github/workflows/release.yml)（多平台 GitHub Release）。

### 项目结构

```
src/
  cli/          # Clap 命令
  engine/       # TradingEngine、订单、成交、market_rules、tradability
  provider/     # yahoo、fcontext、回退链
  tui/          # Ratatui 仪表盘（自适应布局、K 线分页）
  skill.rs      # AgentSkill + schema
data/           # SQLite（gitignore）、测试配置
scripts/
  build_release.sh
  test/         # Shell 集成测试
```

### 架构

```
┌─────────┐   ┌─────────┐
│   CLI   │   │   TUI   │
└────┬────┘   └────┬────┘
     │             │
     └──────┬──────┘
            ▼
     TradingEngine ──► SQLite（账户、订单、持仓）
            │
            ▼
   FallbackProvider（yahoo → fcontext）
```

## 免责声明

**仅供研究与学习。** 本项目为 **模拟盘** 工具，不连接券商、不下真实单、不提供投资建议。行情可能延迟或不准确，使用后果自负。

## 许可证

MIT — 见 [LICENSE](LICENSE)。