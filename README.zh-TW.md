# paper-trading-terminal

**語言：** [English](README.md) · [简体中文](README.zh-CN.md) · 繁體中文 · [日本語](README.ja.md) · [한국어](README.ko.md)

**面向 AI 的美股模擬盤 CLI** — 即時行情、持倉組合與交易。

## 功能

- **模擬帳戶** — 現金、持倉、按市值損益，持久化於 SQLite；TUI 中 `z` 可重置為 `initial_cash` 並清空持倉與訂單
- **訂單** — 市價/限價買賣、撤單，觸及限價自動成交；TUI 掛單顯示成交價與費用
- **貼近真實的模擬** — 依交易時段執行（盤中/盤前盤後/休市）、整手規則、A 股漲跌停與 T+1 賣出鎖定、各市場監管費 + 可設定券商佣金
- **行情** — 優先 Yahoo，失敗時回退 fcontext CLI；兩者皆不可用時會明確報錯
- **TUI** — 自適應版面（小終端自動縮面板與字級）、自選股、Braille K 線整頁翻動、內建下單、成交提醒
- **AI 原生** — 結構化 JSON 輸出、`paper schema` 工具發現、`AgentSkill` Rust 嵌入
- **Rust 函式庫** — 透過 `AgentSkill` 與 `TradingEngine` 嵌入應用

## 環境需求

- **paper** 二進位在 `PATH` 中（見下方 [安裝與執行](#安裝與執行)）
- **fcontext** CLI — *選用*；Yahoo 不可用時作為備用資料源

從原始碼建置另需 Rust stable ≥ 1.91（預設啟用 Yahoo）。

### AI 代理安裝（Claude / Codex / OpenClaw）

可將安裝交給程式代理完成。把 **[docs/agent-install.md](docs/agent-install.md)** 中的提示詞複製到 Claude Code、Codex 或 OpenClaw，由代理執行安裝並確認 `paper quote AAPL` 可用。

<details>
<summary>快速複製 — 通用安裝提示詞</summary>

```text
在本機安裝 paper-trading-terminal CLI（`paper`）。

專案：https://github.com/tsui66/paper-trading-terminal

要求：
- 自行辨識作業系統並執行官方安裝腳本（不要只列印指令）。
- macOS/Linux：curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
- Windows PowerShell：iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

驗證：paper -h → paper config provider-status → paper quote AAPL → paper account。
腳本失敗時改用 Homebrew / Scoop / cargo build --release。
回報安裝路徑與驗證輸出。除非 Yahoo 失敗，否則 fcontext 為選用。
```

更多提示詞（分代理、fcontext 備用、JSON 工具接入）：[docs/agent-install.md](docs/agent-install.md)。

</details>

## 安裝與執行

依序執行。每步終端會有提示，對照確認是否成功。

### 步驟 1 — 安裝 `paper`

依平台選擇 **一條** 指令。

**macOS / Linux（建議）**

```bash
curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

預期輸出類似：

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

應看到 `paper CLI v… installed`；首次可能提示已加入 PATH。**若找不到 `paper`，請重新啟動終端。**

<details>
<summary>其他安裝方式</summary>

**Homebrew（macOS / Linux）**

```bash
brew install --cask tsui66/tap/paper-trading-terminal
```

**Windows（[Scoop](https://scoop.sh)）**

```powershell
scoop install https://github.com/tsui66/paper-trading-terminal/raw/refs/heads/main/.scoop/paper.json
```

**從原始碼建置**（需 Rust ≥ 1.91）

```bash
git clone https://github.com/tsui66/paper-trading-terminal
cd paper-trading-terminal
cargo build --release
# 二進位：./target/release/paper
make install-local   # 選用：複製到 /usr/local/bin
```

Fork 或自建 Release：

```bash
PAPER_INSTALL_REPO=your-org/paper-trading-terminal curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

</details>

### 升級

```bash
paper upgrade --check          # 與 GitHub 最新 release 比對
paper upgrade                  # 下載並替換目前二進位檔
paper upgrade --version v0.0.2 # 安裝指定版本
```

使用 [GitHub Releases](https://github.com/tsui66/paper-trading-terminal/releases)。可用 `PAPER_INSTALL_REPO=owner/name` 或 `--repo` 覆寫倉庫。

### 步驟 2 — 驗證 `paper`

```bash
paper -h
paper config provider-status
```

預期：列印說明；`yahoo` 為 **ok**（主源）。首次執行 `fcontext` 可能為 **missing**，可先做步驟 4。

```bash
paper quote AAPL
```

預期：一行報價，例如 `AAPL  $…  +….%  [yahoo]`。

### 步驟 3 — 執行

```bash
paper account          # 現金與權益
paper tui              # 互動式儀表板
```

TUI 快捷鍵：`j`/`k` 移動自選、`b`/`s` 買賣、`Tab` 切換 K 線週期、`←`/`→` 整頁翻動 K 線（更早/更新）、`z` 雙確認重置帳戶、`q` 離開。

**命令列模擬交易**

```bash
paper buy AAPL --qty 10
paper buy MSFT --qty 5 --limit 500   # 限價單 — 觸及價格後成交
paper orders
paper cancel <order-id-prefix>
paper portfolio --json
```

### 步驟 4 —（選用）安裝 fcontext 備用源

若 Yahoo 已正常可略過。建議在以下情況安裝：

- `paper config provider-status` 顯示 `fcontext: missing` 且需要備用源
- Yahoo 不穩定、報價間歇失敗
- 需要 Yahoo 未涵蓋的標的或資料

`paper` 會自動呼叫 `fcontext`（或 `fctx`），預設 `config.toml` 即可，無需額外設定。

**4a. 安裝 CLI**

macOS（Homebrew）：

```bash
brew install --cask aitaport/tap/fcontext-cli
```

Linux / macOS（腳本）：

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

確認 `fcontext`（及 `fctx`）在 `PATH`：

```bash
fcontext -h
```

**4b. 登入（一次性）**

```bash
fcontext auth login
```

瀏覽器開啟 URL 授權後：

```bash
fcontext auth login --auth-code YOUR_CODE
```

檢查：

```bash
fcontext auth status
fcontext check
```

**4c. 用 `paper` 驗證**

```bash
fcontext quote AAPL.US --format json
paper config provider-status
paper quote AAPL
```

預期：provider-status 中 `fcontext` 為 **ok**；報價可能顯示 `[yahoo]` 或 `[fcontext]`。

`paper` 接受 `AAPL`；fcontext 內部使用 `AAPL.US`。

更多：[fcontext CLI 文件](https://docs.fcontext.com)。

## 速查

全域參數（所有命令）：

| 參數 | 說明 |
|------|------|
| `--json` | 機器可讀 JSON 輸出 |
| `--config PATH` | 設定檔（預設 `./config.toml`） |
| `--db PATH` | SQLite 路徑（預設 `data/paper.db`） |

環境變數：`PAPER_CONFIG` 覆寫設定路徑（舊名：`PPT_CONFIG`）。

## 行情資料源

`config.toml` 預設鏈路：

```toml
[provider]
default = "yahoo"
fallback = ["fcontext"]

[provider.fcontext]
cli = "fcontext"
timeout_secs = 30
```

| 優先順序 | 提供商 | 說明 |
|----------|--------|------|
| 1 | **yahoo** | 預設；免費 Yahoo Finance（可能不穩定） |
| 2 | **fcontext** | 備用 CLI；需安裝並 `fcontext auth login` |

```
yahoo ──失敗──► fcontext ──失敗──► 報錯（中止操作）
```

診斷：

```bash
paper config provider-status
```

## TUI

```bash
paper tui
```

![Paper Trading Terminal TUI](docs/tui-screenshot.png)

儀表板隨終端尺寸自適應：面板寬度、列高、表格字級在小視窗自動緊湊（建議最小約 80×24）。底部快捷鍵在緊湊模式下會縮短。

| 按鍵 | 操作 |
|------|------|
| `j` / `k` 或 `↓` / `↑` | 移動自選高亮 |
| `Enter` | 選中自選標的（載入圖表） |
| `Tab` / `Shift-Tab` | K 線週期（1m … Year）；重置到最新頁 |
| `←` / `→` | K 線翻頁 — **按一次鍵 = 一整屏 K 線**（← 更早，→ 更新） |
| `b` / `s` | 買入 / 賣出目前標的 |
| `m` | 下單列切換市價 / 限價 |
| `Enter` | 送出訂單（下單列啟用時） |
| `Esc` | 取消下單，或取消帳戶重置確認 |
| `n` | 切換選中的掛單 |
| `x` | 撤銷選中的掛單 |
| `z` | 重置帳戶 — 連按兩次確認；恢復 `initial_cash`，清空持倉與訂單 |
| `r` | 重新整理報價與圖表 |
| `q` | 離開 |

**面板：** 自選（左）、K 線（中）、持倉 + 掛單（右）、日誌、下單/快捷鍵列（底）。訂單表含代碼、方向、類型、數量、成交價、費用、狀態；選中列有詳情。

**圖表導覽：** 第 0 頁為最新 K 線。`←` 載入更早一頁，按需拉取歷史。`→` 回到較新頁面。下單列開啟時方向鍵無效（`Esc` 離開）。

限價成交會響鈴並在日誌輸出 `*** FILLED ***` 及費用明細。

## CLI 參考

| 命令 | 說明 |
|------|------|
| `account` | 現金、權益、持倉數 |
| `portfolio` | 按市值組合明細 |
| `positions` | 目前持倉 |
| `quote SYM [SYM…]` | 即時報價 |
| `historical SYM --range m6 --interval d1` | OHLCV K 線 |
| `buy SYM --qty N [--limit P]` | 市價或限價買入 |
| `sell SYM --qty N [--limit P]` | 市價或限價賣出 |
| `orders` | 待成交限價單 |
| `cancel ID` | 依 UUID 或前綴撤單 |
| `history` | 已成交 / 已撤銷紀錄 |
| `pnl` | 已實現 + 未實現損益 |
| `config show` | 目前設定 |
| `config set-provider NAME` | `yahoo` \| `fcontext` |
| `config set-fallback a,b` | 逗號分隔備用清單 |
| `config provider-status` | 探測各資料源與鏈路 |
| `schema` | Agent 整合 schema（JSON） |
| `upgrade` | 下載最新 release 並替換 `paper` 二進位檔 |
| `upgrade --check` | 檢查是否有新版本 |
| `upgrade --version v0.0.2` | 安裝指定 release 標籤 |
| `tui` | 啟動儀表板 |

**區間：** `d1` `d5` `m1` `m3` `m6` `y1` `y5`  
**週期：** `m1` `m5` `m15` `m30` `h1` `d1` `w1` `mo1`

## 交易模擬規則

成交遵循標的後綴與即時行情中的交易時段：

| 規則 | 美股 | 港股 | A 股（`.SH` / `.SZ`） |
|------|------|------|------------------------|
| 整手 | 1 股 | 100 股（預設） | 100 股 |
| T+1 賣出鎖定 | 否 | 否 | 是 — 當日買入次日才可賣 |
| 延長交易 | 允許盤前盤後市價 | 休市時限價可排隊 | 依 A 股時段 |
| 漲跌停 | — | — | 限價 ±10%（ST ±5%） |
| 監管費用 | 賣出 SEC / FINRA | 印花稅、徵費 | 印花稅、過戶費 |

平台佣金可設定；監管費始終計入：

```toml
[trading]
commission_per_trade = 0.0   # 每筆固定
commission_bps = 0.0         # 成交額 bps（1 bps = 0.01%）
min_commission = 0.0
slippage_bps = 5.0
```

**休市** 時拒絕市價單；限價單可排隊。**停牌** 或 **暫停上市** 標的拒絕一切訂單。

## 設定

專案根目錄 `config.toml`（或 `--config` / `PAPER_CONFIG` 指定路徑）：

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

選用環境變數見 `.env.example` 與 `RUST_LOG`。fcontext 認證由 fcontext CLI 管理，非 `paper`。

帳戶重置僅在 TUI（`z` 雙確認）— 尚無 CLI `reset` 命令。

## Agent 與函式庫

**透過 Agent 安裝：** [docs/agent-install.md](docs/agent-install.md) — Claude Code、Codex、OpenClaw 可複製提示詞。

發現 CLI 契約：

```bash
paper schema --json
```

子程序整合範例：

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

## 開發

```bash
make test          # cargo test + CLI 整合測試
make lint          # fmt + clippy
./scripts/test/test_fcontext.sh   # 無 fcontext 時略過
```

本機打包：

```bash
./scripts/package_release.sh                    # 本機 tarball → dist/
./scripts/package_release.sh 0.1.0 darwin-arm64 linux-amd64 windows-amd64
cargo build --no-default-features   # 無 Yahoo 的精簡版
```

推送 `v*` 標籤觸發 [`.github/workflows/release.yml`](.github/workflows/release.yml)（多平台 GitHub Release）。

### 專案結構

```
src/
  cli/          # Clap 命令
  engine/       # TradingEngine、訂單、成交、market_rules、tradability
  provider/     # yahoo、fcontext、回退鏈
  tui/          # Ratatui 儀表板（自適應版面、K 線分頁）
  skill.rs      # AgentSkill + schema
data/           # SQLite（gitignore）、測試設定
scripts/
  build_release.sh
  test/         # Shell 整合測試
```

### 架構

```
┌─────────┐   ┌─────────┐
│   CLI   │   │   TUI   │
└────┬────┘   └────┬────┘
     │             │
     └──────┬──────┘
            ▼
     TradingEngine ──► SQLite（帳戶、訂單、持倉）
            │
            ▼
   FallbackProvider（yahoo → fcontext）
```

## 免責聲明

**僅供研究與學習。** 本專案為 **模擬盤** 工具，不連接券商、不下真實單、不提供投資建議。行情可能延遲或不準確，使用後果自負。

## 授權

MIT — 見 [LICENSE](LICENSE)。