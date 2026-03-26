# XGEN AI 챗봇 통합 플랜 v2 — 캔버스 중심 아키텍처

## 핵심 목표
캔버스(워크플로우 빌더) 화면에서 AI 챗봇이 **워크플로우를 자동 구성**하고,
**문서 인덱싱**, **노드 설정 변경** 등을 대화형으로 수행한다.
페이지 이동 없이 캔버스를 유지하면서 작업하는 것이 주목적.
사용자 요청 시 다른 페이지로 이동할 수도 있다.

---

## 사용 시나리오

### 시나리오 1: 워크플로우 자동 구성
```
사용자: "고객 문의를 분류하고 답변하는 워크플로우 만들어줘"
챗봇:
  1. search_tools("list nodes categories") → 사용 가능한 노드 목록 파악
  2. canvas_add_node("input_string", position={x:100, y:100}) → 입력 노드 추가
  3. canvas_add_node("agents/xgen", position={x:400, y:100}) → 에이전트 노드 추가
  4. canvas_add_node("tools/print_agent_output", position={x:700, y:100}) → 출력 추가
  5. canvas_connect("input_string:output", "agents/xgen:input") → 연결
  6. canvas_connect("agents/xgen:output", "print_agent_output:input") → 연결
  → 캔버스에 3개 노드가 자동 배치되고 연결됨
```

### 시나리오 2: 문서 인덱싱 + RAG 노드 설정
```
사용자: (파일 드래그앤드롭) "이 문서를 인덱싱해줘"
챗봇: "어느 컬렉션에 넣을까요? 기존 컬렉션 목록입니다: ..."
사용자: "새로 만들어줘. '고객매뉴얼'로"
챗봇:
  1. call_tool("create_collection", {name: "고객매뉴얼"}) → 컬렉션 생성
  2. call_tool("index_document", {file: ..., collection: "고객매뉴얼"}) → 인덱싱
  3. canvas_update_node_param("qdrant_node_id", "collection_name", "고객매뉴얼")
     → 캔버스의 RAG 노드에 해당 컬렉션 자동 선택
  → 페이지 이동 없이 캔버스에서 완료
```

### 시나리오 3: 노드 설정 변경
```
사용자: "에이전트 노드의 LLM을 Claude로 바꿔줘"
챗봇:
  1. canvas_get_nodes() → 현재 캔버스의 노드 목록 확인
  2. canvas_update_node_param("agent_node_id", "llm_provider", "anthropic")
  3. canvas_update_node_param("agent_node_id", "model", "claude-sonnet-4-20250514")
  → 캔버스 노드 설정이 실시간 변경
```

### 시나리오 4: 페이지 이동 (부가)
```
사용자: "관리자 설정 페이지로 가줘"
챗봇: navigate("/admin?view=dashboard")
```

---

## 아키텍처

### 전체 구조
```
┌─────────────────────────────────────────────────┐
│  캔버스 페이지 (canvas/page.tsx)                  │
│  ┌──────────────────────┐  ┌──────────────────┐  │
│  │  워크플로우 에디터     │  │  AI 챗봇 패널    │  │
│  │  (노드/엣지/캔버스)   │  │  (사이드 패널)   │  │
│  │                      │  │                  │  │
│  │  ← canvas_* 이벤트 ──│──│── LLM 응답 ──→   │  │
│  │                      │  │                  │  │
│  └──────────────────────┘  └──────────────────┘  │
└──────────────────────┬──────────────────────────┘
                       │ API
        ┌──────────────┴──────────────┐
        │  xgen-backend-gateway       │
        │  /api/ai-chat/* 라우팅      │
        └──────────────┬──────────────┘
                       │
        ┌──────────────┴──────────────┐
        │  xgen-workflow              │
        │  /api/ai-chat/stream        │
        │                             │
        │  graph-tool-call (Python)   │
        │  ├─ search_tools            │
        │  └─ call_tool               │
        │                             │
        │  LangChain/LangGraph agent  │
        │  (LLM + tool_use 루프)      │
        └─────────────────────────────┘
```

### Tool 분류 (5종)

| Tool | 실행 위치 | 설명 |
|------|----------|------|
| **search_tools** | 백엔드 (graph-tool-call) | API 검색 |
| **call_tool** | 백엔드 (graph-tool-call) | API 실행 |
| **canvas_*** | 프론트엔드 (캔버스 상태) | 노드/엣지 CRUD, 파라미터 변경 |
| **navigate** | 프론트엔드 (라우터) | 페이지 이동 |
| **ask_user** | 프론트엔드 (챗봇 UI) | 사용자에게 선택지 제시 |

### Canvas Tools 상세

#### 읽기
| Tool | 설명 | 파라미터 |
|------|------|---------|
| `canvas_get_nodes` | 현재 캔버스의 노드 목록 반환 | — |
| `canvas_get_edges` | 현재 엣지(연결) 목록 반환 | — |
| `canvas_get_node_detail` | 특정 노드의 상세 정보 (파라미터 포함) | node_id |
| `canvas_get_available_nodes` | 추가 가능한 노드 타입 목록 | category? |

#### 쓰기
| Tool | 설명 | 파라미터 |
|------|------|---------|
| `canvas_add_node` | 노드 추가 | node_type, position?, params? |
| `canvas_remove_node` | 노드 삭제 | node_id |
| `canvas_connect` | 두 노드의 포트 연결 | source_node, source_port, target_node, target_port |
| `canvas_disconnect` | 연결 해제 | edge_id |
| `canvas_update_node_param` | 노드 파라미터 값 변경 | node_id, param_name, value |
| `canvas_save` | 워크플로우 저장 | — |

#### 실행
| Tool | 설명 | 파라미터 |
|------|------|---------|
| `canvas_execute` | 현재 워크플로우 실행 | input? |
| `canvas_execute_test` | 테스트 실행 (특정 노드까지) | target_node_id, input? |

---

## 실행 방식: 프론트 vs 백엔드

### 핵심 질문: canvas_* tool은 어디서 실행?

**답: 프론트엔드에서 실행.**

이유:
- 캔버스 상태(노드/엣지)는 React state에 있음 (canvas/page.tsx)
- 노드 추가/삭제/연결은 프론트엔드 상태를 직접 수정해야 함
- 백엔드 API로 워크플로우 저장/로드는 가능하지만, 실시간 캔버스 조작은 불가

### 실행 흐름

```
[데스크탑 앱 (Tauri)]
1. 사용자 입력 → CLI 윈도우 → invoke('cli_send_message')
2. Rust → LLM 호출 (search_tools, call_tool, canvas_*, navigate)
3. search_tools/call_tool → Rust에서 graph-tool-call subprocess로 처리
4. canvas_* → Rust가 메인 윈도우로 Tauri event emit
5. 메인 윈도우(캔버스) → 이벤트 수신 → React state 수정 → UI 업데이트
6. 결과를 Tauri event로 CLI 윈도우에 반환

[웹 (향후)]
1. 사용자 입력 → 채팅 패널 → POST /api/ai-chat/stream
2. 백엔드 → LLM 호출
3. search_tools/call_tool → 백엔드에서 직접 처리
4. canvas_* → SSE 이벤트로 프론트에 전달 → React state 수정
5. navigate → SSE 이벤트로 프론트에 전달 → router.push
```

---

## 관련 레포 및 브랜치

| 레포 | 브랜치 | 작업 |
|------|--------|------|
| **xgen-app** (Tauri) | `main` | canvas_* tool dispatch + 이벤트 emit |
| **xgen-frontend** | `feat/ai-chatbot` | 캔버스에서 canvas_* 이벤트 수신 + 상태 변경 |
| **xgen-workflow** | `feat/ai-chatbot` | 백엔드 AI 챗봇 엔드포인트 (웹용, Phase 2) |
| **xgen-backend-gateway** | `feat/ai-chatbot` | 라우팅 추가 (Phase 2) |

---

## 구현 Phase

### Phase 1: 데스크탑 앱 캔버스 통합 (xgen-app + xgen-frontend 패치)

현재 Tauri 앱의 AI CLI에 canvas_* tool을 추가하여 캔버스 조작 가능하게 한다.

#### 1-1. Rust: canvas_* meta-tool 정의 (tool_search.rs)
- `canvas_get_nodes`, `canvas_get_edges`, `canvas_get_node_detail`
- `canvas_get_available_nodes`
- `canvas_add_node`, `canvas_remove_node`
- `canvas_connect`, `canvas_disconnect`
- `canvas_update_node_param`
- `canvas_save`

#### 1-2. Rust: canvas_* dispatch (llm_client.rs)
- canvas_* tool 호출 시 → Tauri event로 메인 윈도우에 emit
- 메인 윈도우에서 결과를 event로 반환 → tool_result로 LLM에 전달
- **양방향 이벤트**: emit("canvas:command", {action, params}) → listen("canvas:result", callback)

#### 1-3. 프론트엔드 패치: 캔버스 이벤트 핸들러 (patch-canvas-chatbot.js)
캔버스 페이지(canvas/page.tsx)에 이벤트 리스너 주입:

```typescript
// Tauri 이벤트 수신
listen('canvas:command', async (event) => {
    const { requestId, action, params } = event.payload;
    let result;

    switch (action) {
        case 'get_nodes':
            result = canvasNodes.map(n => ({id: n.id, type: n.data.nodeName, ...}));
            break;
        case 'add_node':
            // addNode() 호출 (기존 캔버스 함수)
            result = addNodeToCanvas(params.node_type, params.position);
            break;
        case 'connect':
            result = addEdge(params.source, params.target);
            break;
        case 'update_node_param':
            result = updateNodeParameter(params.node_id, params.param_name, params.value);
            break;
        // ...
    }

    // 결과 반환
    emit('canvas:result', { requestId, result });
});
```

#### 1-4. 캔버스 페이지에 챗봇 패널 추가
- 캔버스 우측에 접이식 채팅 패널
- 기존 cli.html 스타일을 React 컴포넌트로 변환
- 또는 기존 CLI 윈도우를 그대로 사용하되, canvas:command 이벤트만 추가

#### 1-5. system_prompt 업데이트
```
당신은 XGEN 워크플로우 빌더 어시스턴트입니다.
캔버스에서 워크플로우를 구성하고, 노드를 추가/연결/설정합니다.

도구:
- search_tools: XGEN API 검색
- call_tool: API 호출 (문서 인덱싱, 컬렉션 생성 등)
- canvas_get_nodes: 현재 캔버스 노드 목록
- canvas_get_available_nodes: 추가 가능한 노드 타입
- canvas_add_node: 노드 추가
- canvas_connect: 노드 연결
- canvas_update_node_param: 노드 설정 변경
- canvas_save: 워크플로우 저장
- navigate: 페이지 이동 (사용자 요청 시)

작업 순서:
1. 사용자 요청 파악
2. 필요하면 canvas_get_nodes로 현재 상태 확인
3. canvas_add_node로 노드 추가
4. canvas_connect로 연결
5. canvas_update_node_param으로 세부 설정
6. 결과를 한국어로 설명
```

### Phase 2: 웹 통합 (xgen-workflow + xgen-frontend)

데스크탑에서 검증된 기능을 웹으로 확장.

#### 2-1. xgen-workflow: AI 챗봇 엔드포인트
- `POST /api/ai-chat/stream` — SSE 스트리밍 응답
- graph-tool-call Python 라이브러리 + LangChain agent
- search_tools/call_tool은 백엔드에서 직접 처리
- canvas_* 명령은 SSE 이벤트로 프론트에 전달

#### 2-2. xgen-frontend: 캔버스 채팅 패널 (네이티브)
- 패치가 아닌 공식 컴포넌트로 구현
- SSE 수신 → canvas_* 이벤트 처리
- 데스크탑/웹 동일 UI

#### 2-3. xgen-backend-gateway: 라우팅
- `/api/ai-chat/*` → xgen-workflow 프록시

### Phase 3: 고급 기능
- 워크플로우 템플릿 자동 생성 ("고객 응대 봇 만들어줘" → 전체 워크플로우)
- 멀티턴 문서 관리 (파일 업로드 → 컬렉션 선택 → 인덱싱 → RAG 노드 설정)
- 워크플로우 디버깅 ("이 노드가 왜 에러나?" → 로그 분석 → 수정 제안)
- A/B 테스트 ("이 워크플로우를 2개 버전으로 만들어줘")

---

## 기술 과제

### 1. 양방향 이벤트 (Tauri)
- CLI → 메인 윈도우: `emit_to("main", "canvas:command", ...)`
- 메인 윈도우 → CLI: `emit_to("cli", "canvas:result", ...)`
- 비동기 요청/응답 매칭: `requestId`로 페어링
- Rust에서 canvas:result를 기다리는 async 구현 필요 (tokio oneshot channel)

### 2. 캔버스 상태 접근
- canvas/page.tsx가 34000줄짜리 거대 컴포넌트
- 내부 함수(addNode, addEdge 등)에 외부에서 접근하려면:
  - window 전역 객체에 노출하거나
  - React context/store로 분리하거나
  - Tauri 이벤트 핸들러를 컴포넌트 내부에 등록

### 3. 노드 위치 자동 계산
- canvas_add_node 시 position을 LLM이 지정하기 어려움
- 자동 레이아웃 알고리즘 필요 (기존 레이아웃 API 활용 가능)
- `get_workflow_layout_api_workflow_execute_tracker_layout_post` API 활용

### 4. 파일 업로드
- 챗봇에서 파일을 드래그앤드롭 → 인덱싱
- Tauri: dialog 플러그인으로 파일 선택
- 웹: fetch + FormData

### 5. 웹 전환 시 차이
- Tauri: Rust가 LLM 호출 + subprocess
- 웹: 백엔드가 LLM 호출 + Python 라이브러리
- canvas_* 이벤트는 둘 다 프론트에서 처리 (동일)
- 추상화 레이어 필요: `ChatProvider` 인터페이스 (Tauri IPC / REST API)

---

## 구현 우선순위

| 순서 | 작업 | 난이도 | 효과 |
|------|------|--------|------|
| **1** | canvas_get_nodes / canvas_get_available_nodes (읽기) | 낮음 | 기반 |
| **2** | canvas_add_node + canvas_connect (쓰기) | 중간 | 핵심 |
| **3** | canvas_update_node_param (설정 변경) | 중간 | 핵심 |
| **4** | system_prompt + 통합 테스트 | 낮음 | 검증 |
| **5** | 멀티턴 문서 인덱싱 (call_tool + canvas_update) | 높음 | 차별화 |
| **6** | 웹 백엔드 엔드포인트 (Phase 2) | 중간 | 확장 |
| **7** | 캔버스 내장 채팅 패널 UI (Phase 2) | 중간 | UX |

---

## 예상 일정

| Phase | 기간 | 산출물 |
|-------|------|--------|
| Phase 1-1~1-3 | 3-5일 | 데스크탑 앱에서 캔버스 조작 가능 |
| Phase 1-4~1-5 | 2-3일 | 챗봇 패널 UI + 통합 테스트 |
| Phase 2 | 5-7일 | 웹 통합 완료 |
| Phase 3 | 지속 | 고급 기능 추가 |
