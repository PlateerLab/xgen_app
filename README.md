# XGEN App

XGEN AI 플랫폼의 데스크톱 앱. Tauri 기반으로 xgen-frontend를 로컬 번들링하고, AI CLI를 통해 자연어로 XGEN API를 제어할 수 있습니다.

## 아키텍처

```
┌──────────────────────────────────────────────────────┐
│  XGEN Desktop App (Tauri)                            │
│                                                      │
│  ┌─────────────────────┐  ┌───────────────────────┐  │
│  │  XGEN Frontend       │  │  AI CLI Window        │  │
│  │  (Next.js 정적빌드)  │  │  (별도 윈도우)         │  │
│  │                      │  │                       │  │
│  │  워크플로우 편집기    │  │  자연어 입력           │  │
│  │  RAG 관리            │  │  → LLM Tool Use       │  │
│  │  채팅 / 설정         │  │  → XGEN API 자동 호출  │  │
│  └──────────┬───────────┘  └──────────┬────────────┘  │
│             │ HTTP                    │ Tauri IPC      │
│             ▼                         ▼                │
│  ┌─────────────────────────────────────────────────┐  │
│  │  Tauri Rust Backend                              │  │
│  │  - XGEN API Client (reqwest)                     │  │
│  │  - Multi-Provider LLM Client (Claude/GPT/Gemini) │  │
│  │  - CLI Session Manager                           │  │
│  │  - Proxy / Tunnel / Sidecar                      │  │
│  └────────────────────────┬────────────────────────┘  │
└───────────────────────────┼───────────────────────────┘
                            │ HTTPS
                            ▼
                ┌──────────────────────┐
                │  xgen.x2bee.com      │
                │  (Backend Gateway)   │
                └──────────────────────┘
```

## 프로젝트 구조

```
xgen_app/
├── src-tauri/                    # Tauri Rust 백엔드
│   ├── src/
│   │   ├── commands/
│   │   │   ├── cli.rs            # AI CLI IPC 커맨드 (send, history, clear, providers)
│   │   │   ├── proxy.rs          # 로컬 LLM 프록시
│   │   │   ├── settings.rs       # 앱 설정
│   │   │   └── ...
│   │   ├── services/
│   │   │   ├── xgen_api.rs       # XGEN REST API 클라이언트 + Tool 정의
│   │   │   ├── llm_client.rs     # 다중 LLM Provider 클라이언트 (SSE 스트리밍)
│   │   │   └── ...
│   │   ├── state/
│   │   │   └── app_state.rs      # AppState + CliSession
│   │   └── lib.rs                # Tauri 앱 엔트리포인트
│   ├── capabilities/
│   │   └── default.json          # IPC 권한 (main + cli 윈도우)
│   ├── tauri.conf.json           # Tauri 설정
│   └── tests/
│       └── cli_integration.rs    # XGEN API + LLM 통합 테스트
├── src-cli/
│   ├── cli.html                  # AI CLI 독립 윈도우 (vanilla JS)
│   └── cliSection/               # (레거시) React CLI 컴포넌트
├── scripts/
│   ├── build.sh                  # 로컬 빌드 스크립트
│   ├── sync-frontend.sh          # GitLab에서 xgen-frontend clone
│   ├── patch-frontend.sh         # Tauri 정적 빌드용 패치
│   └── patch-sidebar-cli.js      # 사이드바 AI CLI 버튼 주입
├── frontend/                     # (빌드 시 자동 생성) xgen-frontend 소스
└── .github/workflows/
    └── build-windows.yml         # CI: Windows + macOS 빌드
```

## AI CLI

사이드바의 **⚡ AI CLI** 버튼을 클릭하면 별도 터미널 창이 열립니다.

### 기능
- 자연어로 XGEN API 제어
- LLM Tool Use로 자동 API 호출 (워크플로우, 스케줄, 노드, 도구 등)
- SSE 스트리밍 실시간 응답
- XGEN 백엔드의 LLM 설정을 자동으로 사용 (별도 API 키 불필요)

### 지원 LLM Provider

| Provider | API 형식 | 모델 예시 |
|----------|---------|----------|
| **Anthropic** (기본) | Claude Messages API | claude-sonnet-4-20250514 |
| **OpenAI** | Chat Completions API | gpt-4o-2024-11-20 |
| **Gemini** | Google AI API | gemini-2.5-flash |
| **vLLM / SGL** | OpenAI 호환 API | (커스텀 모델) |

드롭다운에서 provider를 전환할 수 있으며, XGEN 백엔드에 설정된 provider만 표시됩니다.

### 지원 Tool (XGEN API)

| Tool | 설명 |
|------|------|
| `list_workflows` | 워크플로우 목록 조회 |
| `get_workflow` | 워크플로우 상세 조회 |
| `save_workflow` | 워크플로우 생성/수정 |
| `execute_workflow` | 워크플로우 실행 |
| `list_schedules` | 스케줄 목록 조회 |
| `create_schedule` | 스케줄 생성 (cron) |
| `list_nodes` | 노드/에이전트 목록 |
| `list_tools` | 등록된 도구 목록 |
| `get_llm_status` | LLM 상태 확인 |

## 빌드

### 사전 요구사항

- Node.js 20+
- Rust ([rustup.rs](https://rustup.rs/))
- Tauri CLI: `cargo install tauri-cli --version "^2"`

### 로컬 빌드

```bash
# 전체 빌드 (프론트 동기화 → 패치 → 빌드)
./scripts/build.sh

# 개발 모드 (핫리로드)
./scripts/build.sh --dev

# 프론트 동기화 건너뛰기 (이미 있을 때)
./scripts/build.sh --skip-sync
```

### CI 빌드 (GitHub Actions)

`main` 브랜치에 push하면 자동으로 Windows + macOS 빌드가 실행됩니다.

빌드 파이프라인:
```
GitLab clone (xgen-frontend)
  → patch-frontend.sh (API Routes 제거, 정적 export 패치, CLI 주입)
  → npm install & build
  → cli.html 복사
  → cargo tauri build
  → 아티팩트 업로드 (DMG, MSI, NSIS)
```

### 빌드 결과물

| 플랫폼 | 파일 |
|--------|------|
| macOS (Apple Silicon) | `XGEN_x.x.x_aarch64.dmg` |
| Windows | `XGEN_x.x.x_x64_en-US.msi` / `XGEN_x.x.x_x64-setup.exe` |

## 설치

### macOS

1. DMG 다운로드 → 앱을 Applications에 드래그
2. 터미널에서 Gatekeeper 해제:
   ```bash
   find /Applications/XGEN.app -exec xattr -c {} \;
   ```
3. 앱 실행

### Windows

MSI 또는 NSIS 설치 파일 실행.

## 환경 변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `FRONTEND_BRANCH` | main | 빌드할 xgen-frontend 브랜치 |
| `NEXT_PUBLIC_BACKEND_HOST` | https://xgen.x2bee.com | 백엔드 API 호스트 |
| `NEXT_PUBLIC_BACKEND_PORT` | (없음) | 백엔드 API 포트 |
| `TAURI_ENV` | true | Tauri 정적 export 활성화 |
| `GITLAB_TOKEN` | - | GitLab clone용 토큰 (CI secret) |

## 개발

### Rust 테스트

```bash
cd src-tauri

# 통합 테스트 (XGEN API + LLM 연동)
cargo test --test cli_integration -- --nocapture

# 컴파일 체크
cargo check
```

### 프론트 패치 테스트

```bash
# 패치 스크립트 단독 실행
FRONTEND_DIR=/path/to/frontend bash scripts/patch-frontend.sh
```

## 릴리즈

[GitHub Releases](https://github.com/PlateerLab/xgen_app/releases)에서 최신 빌드를 다운로드할 수 있습니다.
