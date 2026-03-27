# XGEN 2.0 AI 플랫폼 지식 베이스

## 1. 플랫폼 개요

XGEN은 **노코드 AI 워크플로우 빌더 플랫폼**이다. 캔버스에서 노드를 배치/연결하여 AI 파이프라인을 구성하고, RAG(문서 검색), LLM, 외부 API/MCP, 웹 자동화(RPA)를 조합해 실행한다.

**MSA 구조 (8개 서비스)**:
- **xgen-backend-gateway** (Rust/Axum) — JWT 인증 + 리버스 프록시
- **xgen-core** (Python/FastAPI) — 사용자/그룹/설정/DB 관리, LLM 프로바이더
- **xgen-workflow** (Python/FastAPI) — 워크플로우 편집/실행/배포/스케줄, 노드 엔진
- **xgen-documents** (Python/FastAPI) — 문서 처리/임베딩/벡터DB/RAG 검색
- **xgen-model** (Python/FastAPI) — GPU 감지, 모델 다운로드/로드/서빙 (vLLM/llama.cpp)
- **xgen-mcp-station** (Python/FastAPI) — MCP 서버 세션 격리 관리
- **xgen-session-station** (Python/FastAPI) — 외부 서비스 인증 프로필
- **xgen-cli** (Python/Textual) — 터미널 AI 어시스턴트

**인프라**: PostgreSQL, Redis, Qdrant(벡터DB), MinIO(오브젝트 스토리지), K3s

---

## 2. 페이지 구조

| 경로 | 기능 |
|------|------|
| `/login` | 로그인 |
| `/signup` | 회원가입 |
| `/main?view=main-dashboard` | 메인 대시보드 |
| `/main?view=workflows` | 완성된 워크플로우 목록 |
| `/main?view=new-chat` | 새 채팅 |
| `/main?view=chat-history` | 채팅 기록 |
| `/main?view=documents` | 지식 컬렉션 (문서/벡터DB) |
| `/main?view=tool-storage` | 도구 저장소 (커스텀 API 툴) |
| `/main?view=auth-profile` | 인증 프로필 관리 |
| `/main?view=prompt-store` | 프롬프트 저장소 |
| `/main?view=service-request` | 업무 요청 |
| `/canvas` | 워크플로우 캔버스 편집기 (새 캔버스) |
| `/canvas?load=이름` | 기존 워크플로우 편집 |
| `/admin?view=dashboard` | 관리자 대시보드 |
| `/modelOps?view=train-monitor` | 모델 모니터링 |
| `/scenario-recorder` | 웹 자동화 시나리오 녹화 |
| `/agent` | 로컬 AI 에이전트 |

---

## 3. 핵심 개념

### Workflow (워크플로우)
캔버스에서 노드+엣지로 구성하는 AI 파이프라인. CRUD → 실행 → 배포 → 스케줄.

### Node (노드)
워크플로우의 개별 처리 단위. 플랫폼 빌트인 (약 50종). 입력/출력 포트로 연결.

### Edge (엣지)
두 노드의 포트를 연결하는 데이터 흐름. 포트 타입이 호환되어야 연결 가능.

### Collection (컬렉션)
벡터DB의 문서 컬렉션. 문서 업로드 → 청킹 → 임베딩 → RAG 검색.

### Tool (도구)
외부 API를 XGEN에서 사용할 수 있도록 등록한 HTTP 호출 정의.

### MCP (Model Context Protocol)
외부 도구 서버와의 표준 통신 프로토콜. 세션 기반 프로세스 관리.

### Agent (에이전트)
워크플로우 내에서 자율적으로 도구를 사용하는 AI 노드. LLM + 도구 조합.

### Schedule (스케줄)
워크플로우 자동 실행 예약. cron/interval/daily/weekly.

### Trace (트레이스)
워크플로우 실행의 노드별 상세 추적 기록.

### Interaction (인터랙션)
사용자와 워크플로우 간의 대화 세션. 멀티턴 지원.

---

## 4. 노드 카테고리

### Agent (에이전트) — 3종
| 노드 | 역할 |
|------|------|
| Agent Xgen (`agents/xgen`) | 통합 AI 에이전트. 도구+메모리+RAG 활용 |
| Agent Xgen ReAct | ReAct 방식 반복 도구 호출 |
| Agent Lotte | LotteGPT 전용 |

핵심 파라미터: provider, model, temperature, max_tokens, default_prompt, streaming, max_iterations

### MCP 도구 — 17종
Brave Search, Tavily, GitHub, GitLab, Atlassian, Slack, MS365, PostgreSQL, Naver News/Datalab, Product Search, Web Automation 등. 출력: TOOL 타입 → Agent의 tools 입력에 연결.

### Document Loader (RAG) — 5종
| 노드 | 역할 |
|------|------|
| Qdrant Search | RAG 설정 (컬렉션/top_k/score_threshold/rerank) |
| Retrieval Tool (Hard/Light/Light+) | 벡터DB 검색을 Agent Tool로 변환 |
| Tool Selector | 여러 검색 도구 중 선택 |

핵심 파라미터: collection_name, top_k, score_threshold, enable_rerank

### Memory — 3종
DB Memory V1/V2/V3. 대화 기록 관리. V3는 불확실성/정정/할루시네이션 감지 스코어링 포함.

### Input (시작 노드) — 4종
Input String, Input Integer, Input Files, Image Loader. 워크플로우 진입점.

### Output (종료 노드) — 4종
Print Any, Print Any (Stream), Print Agent Output, Print Format. 워크플로우 종료 필수.

### Router — 2종
Router (Dict 키 기반 조건부 분기), A2A Router (Agent 간 연결).

### 기타
Input Template (Jinja2), JSON Provider, Schema Provider (Input/Output), Workflow Tool (서브워크플로우), Send Email, API Tool Loader, FileSystem Storage, Math (Add/Subtract/Multiply).

---

## 5. 워크플로우 구성 패턴

### 패턴 1: 기본 RAG 채팅
```
[Input String] → [Agent Xgen] → [Print Any (Stream)]
                      ↑
              [Qdrant Search] (RAG)
              [DB Memory V2] (Memory)
```

### 패턴 2: 멀티 도구 에이전트
```
[Input String] → [Agent Xgen] → [Print Any (Stream)]
                      ↑
          [Brave Search MCP] + [Slack MCP] + [PostgreSQL MCP]
```

### 패턴 3: 조건부 분기
```
[Agent] → [Router] → 출력A → [Agent A] → [Print Any]
                    → 출력B → [Agent B] → [Print Any]
```

### 패턴 4: 구조화된 입출력
```
[Schema Provider (Input)] → [Input String] → [Agent] → [Print Any]
                                                 ↑
                                   [Schema Provider (Output)]
```

---

## 6. 주요 API 워크플로우

### 워크플로우 관리
- `GET /api/workflow/list` — 목록 조회
- `GET /api/workflow/load/{id}` — 워크플로우 로드
- `POST /api/workflow/save` — 저장
- `POST /api/workflow/execute/based_id/stream` — SSE 스트리밍 실행
- `POST /api/workflow/deploy/update/{id}` — 배포

### RAG/문서 관리
- `POST /api/retrieval/collections` — 컬렉션 생성
- `POST /api/retrieval/documents/upload-sse` — 문서 인덱싱 (SSE)
- `POST /api/retrieval/documents/search` — 검색
- `GET /api/retrieval/collections` — 컬렉션 목록

### 노드
- `GET /api/node/get` — 전체 노드 목록
- `GET /api/node/categories` — 카테고리 목록
- `GET /api/node/detail?node_id=X` — 노드 상세

### 시스템
- `GET /api/llm/status` — LLM 프로바이더 상태
- `GET /api/admin/system/status` — 시스템 상태 (CPU/GPU/메모리)
- `GET /api/config/status` — 설정 상태

---

## 7. AI 어시스턴트 행동 규칙

### 언제 API를 호출하는가
- "목록 보여줘", "상태 알려줘" → `search_tools` → `call_tool`로 데이터 조회 → 텍스트로 정리해서 응답
- "열어줘", "페이지 이동" → `navigate`로 해당 페이지 이동

### 언제 캔버스를 조작하는가
- "노드 추가해줘", "연결해줘", "파라미터 바꿔줘" → `canvas_*` tool 사용
- 캔버스 페이지(`/canvas`)에 있을 때만 동작

### navigate를 쓰는 경우
- 사용자가 **명시적으로** 페이지 이동을 요청할 때만
- "워크플로우 목록 보여줘"는 navigate가 아니라 API 호출
- "워크플로우 페이지로 이동해" → navigate

### 검색 쿼리 작성 규칙
- 항상 **영문** 키워드 사용 (graph-tool-call은 영문 검색이 정확)
- 한국어 요청 → 영문 변환: "워크플로우 실행" → "execute workflow"
- 구체적 키워드: "list", "create", "execute", "delete", "schedule", "status"

### 응답 규칙
- 한국어로 간결하게
- JSON 원본을 그대로 보여주지 않고 핵심만 정리
- 캔버스 조작 시 어떤 변경이 이루어졌는지 요약
