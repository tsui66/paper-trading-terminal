# paper-trading-terminal

**AI 네이티브 미국 주식 페이퍼 트레이딩 CLI** — 실시간 시세, 포트폴리오, 거래.

<p align="center"><strong>언어:</strong> <a href="README.md">English</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · 한국어</p>

## 기능

- **페이퍼 계좌** — 현금, 포지션, 시가 평가 손익을 SQLite에 저장; TUI에서 `z`로 `initial_cash` 복원 및 포지션·주문 초기화
- **주문** — 시장가/지정가 매매, 취소, 지정가 도달 시 자동 체결; TUI에서 체결가와 수수료 표시
- **현실에 가까운 시뮬레이션** — 세션 인식(정규/프리·애프터/휴장), 로트 크기, A주 가격 밴드 및 T+1 매도 잠금, 시장별 규제 수수료 + 설정 가능한 브로커 커미션
- **시세** — Yahoo 우선, 실패 시 fcontext CLI 폴백; 둘 다 불가 시 명확한 오류
- **TUI** — 터미널 크기에 맞는 레이아웃, 관심종목, Braille 캔들 차트 페이지 넘김, 앱 내 주문, 체결 알림
- **AI 네이티브** — 구조화 JSON I/O, `paper schema` 도구 탐색, Rust 임베딩용 `AgentSkill`
- **Rust 라이브러리** — `AgentSkill`과 `TradingEngine`으로 임베딩

## 요구 사항

- **paper** 바이너리가 `PATH`에 있어야 함（아래 [설치 및 실행](#설치-및-실행) 참고）
- **fcontext** CLI — *선택*; Yahoo 불가 시 폴백

소스 빌드 시 Rust stable ≥ 1.91 필요（Yahoo 기본 활성화）.

### AI 에이전트 설치（Claude / Codex / OpenClaw）

코딩 에이전트에게 설치와 검증을 맡길 수 있습니다. **[docs/agent-install.md](docs/agent-install.md)** 의 프롬프트를 Claude Code, Codex, OpenClaw에 붙여넣고 `paper quote AAPL`이 동작할 때까지 실행하게 하세요.

<details>
<summary>빠른 복사 — 범용 설치 프롬프트</summary>

```text
이 머신에 paper-trading-terminal CLI（`paper`）를 설치하세요.

프로젝트：https://github.com/tsui66/paper-trading-terminal

규칙：
- OS를 스스로 판별하고 공식 설치 스크립트를 실행할 것（명령만 출력하지 말 것）.
- macOS/Linux：curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
- Windows PowerShell：iwr https://github.com/tsui66/paper-trading-terminal/raw/main/install.ps1 | iex

검증：paper -h → paper config provider-status → paper quote AAPL → paper account.
스크립트 실패 시 Homebrew / Scoop / cargo build --release 시도.
설치 경로와 검증 결과 보고. Yahoo 실패가 아니면 fcontext는 선택.
```

에이전트별·fcontext·JSON 연동：[docs/agent-install.md](docs/agent-install.md).

</details>

## 설치 및 실행

순서대로 진행하세요. 각 단계 출력으로 성공 여부를 확인합니다.

### 1단계 — `paper` 설치

플랫폼에 맞는 **한 가지** 명령을 실행하세요.

**macOS / Linux（권장）**

```bash
curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

예상 출력：

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

`paper CLI v… installed`가 표시됩니다. 처음에는 PATH 추가 메시지가 나올 수 있습니다. **`paper`를 찾을 수 없으면 터미널을 재시작하세요.**

<details>
<summary>기타 설치 방법</summary>

**Homebrew（macOS / Linux）**

```bash
brew install --cask tsui66/tap/paper-trading-terminal
```

**Windows（[Scoop](https://scoop.sh)）**

```powershell
scoop install https://github.com/tsui66/paper-trading-terminal/raw/refs/heads/main/.scoop/paper.json
```

**소스 빌드**（Rust ≥ 1.91）

```bash
git clone https://github.com/tsui66/paper-trading-terminal
cd paper-trading-terminal
cargo build --release
# 바이너리：./target/release/paper
make install-local   # 선택：/usr/local/bin에 복사
```

Fork 또는 자체 호스팅：

```bash
PAPER_INSTALL_REPO=your-org/paper-trading-terminal curl -sSL https://github.com/tsui66/paper-trading-terminal/raw/main/install | sh
```

</details>

### 업그레이드

```bash
paper upgrade --check          # GitHub 최신 릴리스와 비교
paper upgrade                  # 현재 바이너리 다운로드 후 교체
paper upgrade --version v0.0.2 # 특정 버전 설치
```

[GitHub Releases](https://github.com/tsui66/paper-trading-terminal/releases) 사용. `PAPER_INSTALL_REPO=owner/name` 또는 `--repo`로 저장소 재정의 가능.

### 2단계 — `paper` 검증

```bash
paper -h
paper config provider-status
```

예상：도움말 출력; `yahoo`가 **ok**（기본）. 첫 실행 시 `fcontext`가 **missing**이어도 괜찮습니다（4단계는 선택）.

```bash
paper quote AAPL
```

예상：`AAPL  $…  +….%  [yahoo]` 형태의 한 줄.

### 3단계 — 실행

```bash
paper account          # 현금 및 자산
paper tui              # 대화형 대시보드
```

TUI：`j`/`k` 관심종목 이동, `b`/`s` 매매, `Tab` 차트 주기, `←`/`→` 차트 페이지（과거/최신）, `z` 계좌 초기화（이중 확인）, `q` 종료.

**CLI 페이퍼 트레이딩**

```bash
paper buy AAPL --qty 10
paper buy MSFT --qty 5 --limit 500   # 지정가 — 가격 도달 시 체결
paper orders
paper cancel <order-id-prefix>
paper portfolio --json
```

### 4단계 —（선택）fcontext 폴백

Yahoo가 정상이면 생략 가능. 다음 경우 설치：

- 백업 소스가 필요（`fcontext: missing`）
- Yahoo 불안정·간헐적 실패
- Yahoo에 없는 종목·데이터 필요

`paper`가 `fcontext`（또는 `fctx`）를 자동 호출합니다. 기본 `config.toml`로 충분합니다.

**4a. CLI 설치**

macOS（Homebrew）：

```bash
brew install --cask aitaport/tap/fcontext-cli
```

Linux / macOS（스크립트）：

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

`fcontext`（및 `fctx`）가 `PATH`에 있는지 확인：

```bash
fcontext -h
```

**4b. 로그인（1회）**

```bash
fcontext auth login
```

브라우저에서 URL 열고 승인 후：

```bash
fcontext auth login --auth-code YOUR_CODE
```

확인：

```bash
fcontext auth status
fcontext check
```

**4c. `paper`로 검증**

```bash
fcontext quote AAPL.US --format json
paper config provider-status
paper quote AAPL
```

예상：provider-status에서 `fcontext` **ok**; 시세는 `[yahoo]` 또는 `[fcontext]`.

`paper`는 `AAPL` 수용; fcontext 내부는 `AAPL.US`.

자세히：[fcontext CLI 문서](https://docs.fcontext.com).

## 빠른 참조

전역 플래그（모든 명령）：

| 플래그 | 설명 |
|--------|------|
| `--json` | 기계 판독 가능 JSON 출력 |
| `--config PATH` | 설정 파일（기본 `./config.toml`） |
| `--db PATH` | SQLite 경로（기본 `data/paper.db`） |

환경 변수：`PAPER_CONFIG`로 설정 경로 재정의（구명：`PPT_CONFIG`）.

## 시세 제공자

`config.toml` 기본 체인：

```toml
[provider]
default = "yahoo"
fallback = ["fcontext"]

[provider.fcontext]
cli = "fcontext"
timeout_secs = 30
```

| 우선순위 | 제공자 | 비고 |
|----------|--------|------|
| 1 | **yahoo** | 기본; 무료 Yahoo Finance（불안정할 수 있음） |
| 2 | **fcontext** | 폴백 CLI; 설치 + `fcontext auth login` |

```
yahoo ──실패──► fcontext ──실패──► 오류（작업 중단）
```

진단：

```bash
paper config provider-status
```

## TUI

```bash
paper tui
```

![Paper Trading Terminal TUI](docs/tui-screenshot.png)

터미널 크기에 맞게 패널 너비·행 높이·표 글자 크기가 자동 조정됩니다（권장 최소 약 80×24）. 컴팩트 모드에서는 하단 단축키 힌트가 짧아집니다.

| 키 | 동작 |
|----|------|
| `j` / `k` 또는 `↓` / `↑` | 관심종목 선택 이동 |
| `Enter` | 하이라이트 종목 선택（차트 로드） |
| `Tab` / `Shift-Tab` | 차트 주기（1m … Year）; 최신 페이지로 리셋 |
| `←` / `→` | 차트 페이지 — **키 한 번 = 화면 가득 봉**（← 과거, → 최신） |
| `b` / `s` | 선택 종목 매수 / 매도 |
| `m` | 주문 바에서 시장가 / 지정가 전환 |
| `Enter` | 주문 제출（주문 바 활성 시） |
| `Esc` | 주문 취소 또는 계좌 초기화 확인 취소 |
| `n` | 대기 주문 선택 순환 |
| `x` | 선택한 대기 주문 취소 |
| `z` | 계좌 초기화 — 두 번 눌러 확인; `initial_cash` 복원, 포지션·주문 삭제 |
| `r` | 시세·차트 새로고침 |
| `q` | 종료 |

**패널：** 관심종목（좌）, 캔들 차트（중）, 보유 + 대기 주문（우）, 로그, 주문/단축키 바（하）. 주문 표에 종목·매매·유형·수량·체결가·수수료·상태 포함.

**차트 탐색：** 0페이지가 최신 봉. `←`로 과거 페이지 로드（필요 시 이력 fetch）. `→`로 최신 쪽 이동. 주문 바 열린 동안 방향키 무시（`Esc`로 종료）.

지정가 체결 시 터미널 벨과 `*** FILLED ***` 로그（수수료 내역 포함）.

## CLI 참조

| 명령 | 설명 |
|------|------|
| `account` | 현금, 자산, 포지션 수 |
| `portfolio` | 시가 평가 내역 |
| `positions` | 보유 포지션 |
| `quote SYM [SYM…]` | 실시간 시세 |
| `historical SYM --range m6 --interval d1` | OHLCV 캔들 |
| `buy SYM --qty N [--limit P]` | 시장가 또는 지정가 매수 |
| `sell SYM --qty N [--limit P]` | 시장가 또는 지정가 매도 |
| `orders` | 대기 중 지정가 주문 |
| `cancel ID` | UUID 또는 접두사로 취소 |
| `history` | 체결 / 취소 기록 |
| `pnl` | 실현 + 미실현 손익 |
| `config show` | 현재 설정 |
| `config set-provider NAME` | `yahoo` \| `fcontext` |
| `config set-fallback a,b` | 쉼표 구분 폴백 목록 |
| `config provider-status` | 제공자·체인 프로브 |
| `schema` | 에이전트 통합 schema（JSON） |
| `upgrade` | 최신 릴리스 다운로드 후 `paper` 바이너리 교체 |
| `upgrade --check` | 새 버전 사용 가능 여부 확인 |
| `upgrade --version v0.0.2` | 특정 릴리스 태그 설치 |
| `tui` | 대시보드 실행 |

**범위：** `d1` `d5` `m1` `m3` `m6` `y1` `y5`  
**간격：** `m1` `m5` `m15` `m30` `h1` `d1` `w1` `mo1`

## 거래 시뮬레이션

체결은 종목 접미사와 실시간 시세의 세션 상태를 따릅니다：

| 규칙 | 미국 | 홍콩 | A주（`.SH` / `.SZ`） |
|------|------|------|----------------------|
| 로트 | 1주 | 100주（기본） | 100주 |
| T+1 매도 잠금 | 없음 | 없음 | 있음 — 익 세션까지 매도 불가 |
| 연장 거래 | 프리/애프터 시장가 허용 | 휴장 시 지정가 대기 | 중국 세션 준수 |
| 가격 밴드 | — | — | 지정가 ±10%（ST ±5%） |
| 규제 수수료 | 매도 SEC / FINRA | 인지세, 부과금 | 인지세, 명의개서료 |

플랫폼 커미션은 설정 가능; 규제 수수료는 항상 반영：

```toml
[trading]
commission_per_trade = 0.0   # 주문당 정액
commission_bps = 0.0         # 명목 bps（1 bps = 0.01%）
min_commission = 0.0
slippage_bps = 5.0
```

**휴장** 시 시장가 거부; 지정가는 대기 가능. **거래정지**·**상장폐지 위험** 종목은 모든 주문 거부.

## 설정

프로젝트 루트 `config.toml`（또는 `--config` / `PAPER_CONFIG`）：

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

`.env.example`에서 환경 변수와 `RUST_LOG`. fcontext 인증은 fcontext CLI가 관리（`paper` 아님）.

계좌 초기화는 TUI만（`z` 이중 확인）— CLI `reset` 명령은 아직 없음.

## 에이전트 및 라이브러리

**에이전트로 설치：** [docs/agent-install.md](docs/agent-install.md) — Claude Code, Codex, OpenClaw용 프롬프트.

CLI 계약 탐색：

```bash
paper schema --json
```

서브프로세스 연동 예：

```bash
paper portfolio --json
paper buy AAPL --qty 10 --json
```

Rust 임베딩：

```rust
use paper_trading_terminal::cli::AppState;
use paper_trading_terminal::skill::{agent_schema, AgentSkill};
use paper_trading_terminal::{create_provider_stack, AppConfig, QuoteCache};

let config = AppConfig::load(None)?;
let provider = create_provider_stack(&config, Some(QuoteCache::new(true, 60)));
let skill = AgentSkill::new(AppState::new(config, provider));
let _schema = agent_schema();
```

## 개발

```bash
make test          # cargo test + CLI 통합
make lint          # fmt + clippy
./scripts/test/test_fcontext.sh   # fcontext 없으면 스킵
```

로컬 릴리스 패키징：

```bash
./scripts/package_release.sh                    # 호스트 tarball → dist/
./scripts/package_release.sh 0.1.0 darwin-arm64 linux-amd64 windows-amd64
cargo build --no-default-features   # Yahoo 없는 슬림 빌드
```

`v*` 태그로 [`.github/workflows/release.yml`](.github/workflows/release.yml) 트리거（멀티 플랫폼 Release）.

### 프로젝트 구조

```
src/
  cli/          # Clap 명령
  engine/       # TradingEngine, 주문, 체결, market_rules, tradability
  provider/     # yahoo, fcontext, 폴백 체인
  tui/          # Ratatui 대시보드（적응 레이아웃, K선 페이지）
  skill.rs      # AgentSkill + schema
data/           # SQLite（gitignore）, 테스트 설정
scripts/
  build_release.sh
  test/         # Shell 통합 테스트
```

### 아키텍처

```
┌─────────┐   ┌─────────┐
│   CLI   │   │   TUI   │
└────┬────┘   └────┬────┘
     │             │
     └──────┬──────┘
            ▼
     TradingEngine ──► SQLite（계좌, 주문, 포지션）
            │
            ▼
   FallbackProvider（yahoo → fcontext）
```

## 면책 조항

**연구 및 학습 목적으로만 사용하세요.** 본 프로젝트는 **페이퍼 트레이딩** 시뮬레이터입니다. 증권사에 연결하지 않으며 실제 주문을 실행하지 않습니다. 투자 조언이 아닙니다. 시세는 지연되거나 부정확할 수 있습니다. 사용에 대한 책임은 사용자에게 있습니다.

## 라이선스

MIT — [LICENSE](LICENSE) 참조.