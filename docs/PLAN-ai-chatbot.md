# XGEN AI 챗봇 통합 플랜

## 목표
XGEN 플랫폼에 AI 챗봇을 내장하여 자연어로 API를 제어할 수 있게 한다.
웹과 데스크탑 앱 모두에서 동일하게 동작해야 한다.

## 현재 상태
- 데스크탑 앱(Tauri)에서만 AI CLI 동작 (별도 윈도우)
- Rust에서 LLM 호출 + graph-tool-call subprocess → 버그 많았음
- v0.4.0에서 gateway 모드(search_tools + call_tool) 전환 완료

## 목표 아키텍처

```
[웹 브라우저]                    [데스크탑 앱]
xgen-frontend                   xgen-frontend (Tauri 내장)
    │                                │
    └──── /api/ai-chat/stream ───────┘
                │
    xgen-backend-gateway (Rust, 라우팅)
                │
    xgen-workflow (Python, AI 챗봇 엔드포인트)
        ├─ graph-tool-call (Python 라이브러리)
        │   ├─ create_gateway_tools() → search_tools + call_tool
        │   └─ OpenAPI spec 기반 동적 API 검색/호출
        ├─ LangChain agent (LLM 호출 + tool_use 루프)
        └─ SSE 스트리밍 응답
```

## 관련 레포 및 브랜치

| 레포 | 브랜치 | 작업 |
|------|--------|------|
| xgen-frontend | `feat/ai-chatbot` | 채팅 UI 컴포넌트 (기존 /chat 페이지 활용) |
| xgen-workflow | `feat/ai-chatbot` | AI 챗봇 API 엔드포인트 + graph-tool-call 통합 |
| xgen-backend-gateway | `feat/ai-chatbot` | /api/ai-chat/* 라우팅 추가 |

## 상세 작업

### 1. xgen-workflow (Python 백엔드)

#### 1-1. graph-tool-call 의존성 추가
```toml
# pyproject.toml
[tool.poetry.dependencies]
graph-tool-call = {version = ">=0.18.0", extras = ["mcp"]}
langchain-anthropic = ">=0.2.0"  # 또는 langchain-openai
```

#### 1-2. AI 챗봇 서비스 (`service/ai_chat.py`)
```python
from graph_tool_call import ToolGraph
from graph_tool_call.langchain import create_gateway_tools
from langchain_anthropic import ChatAnthropic
from langgraph.prebuilt import create_react_agent

class AiChatService:
    def __init__(self, openapi_source: str, llm_config: dict):
        # OpenAPI에서 tool graph 빌드
        self.graph = ToolGraph.from_openapi(openapi_source)
        all_tools = self.graph.to_langchain_tools(base_url=..., auth_token=...)

        # Gateway tools (search_tools + call_tool)
        self.gateway_tools = create_gateway_tools(all_tools, top_k=7)

        # LLM
        self.llm = ChatAnthropic(model=llm_config["model"], api_key=llm_config["api_key"])

        # Agent
        self.agent = create_react_agent(self.llm, tools=self.gateway_tools)

    async def chat_stream(self, messages: list, auth_token: str):
        """SSE 스트리밍으로 응답 반환"""
        async for event in self.agent.astream_events(
            {"messages": messages}, version="v2"
        ):
            yield format_sse_event(event)
```

#### 1-3. AI 챗봇 컨트롤러 (`controller/ai_chat_controller.py`)
```python
@router.post("/ai-chat/stream")
async def ai_chat_stream(request: AiChatRequest, token: str = Depends(get_current_user)):
    service = get_ai_chat_service(auth_token=token)
    return StreamingResponse(
        service.chat_stream(request.messages, auth_token=token),
        media_type="text/event-stream"
    )

@router.get("/ai-chat/providers")
async def list_providers():
    """사용 가능한 LLM 프로바이더 목록"""
    ...
```

### 2. xgen-backend-gateway (Rust)

#### 2-1. 라우팅 추가
```
/api/ai-chat/stream    → xgen-workflow:8000/ai-chat/stream (SSE proxy)
/api/ai-chat/providers → xgen-workflow:8000/ai-chat/providers
/api/ai-chat/history   → xgen-workflow:8000/ai-chat/history
```

### 3. xgen-frontend (Next.js)

#### 3-1. AI 챗봇 컴포넌트
- 기존 `/chat` 페이지의 채팅 UI 재사용 또는 새 컴포넌트
- 플로팅 버튼 (화면 우하단) → 클릭 시 채팅 패널 열기
- SSE 스트리밍 수신 → 실시간 응답 표시
- tool 호출 표시 (🔍 검색 / ⚡ 호출)

#### 3-2. API 연동
```typescript
// EventSource로 SSE 수신
const response = await fetch('/api/ai-chat/stream', {
  method: 'POST',
  headers: { 'Authorization': `Bearer ${token}` },
  body: JSON.stringify({ messages }),
});
const reader = response.body.getReader();
// 스트리밍 처리...
```

#### 3-3. Tauri 앱 통합
- 데스크탑 앱에서도 동일한 웹 컴포넌트 사용
- 별도 CLI 윈도우 대신 내장 채팅 패널
- `src-cli/cli.html` → 폐기 (웹 컴포넌트로 대체)

## 데스크탑 앱 변경사항

### xgen-app (Tauri)
v0.4.0의 Rust LLM 클라이언트는 **백엔드 API 호출로 대체**:
- `llm_client.rs` → 삭제 가능 (백엔드가 LLM 호출)
- `tool_search.rs` → 삭제 가능 (백엔드가 graph-tool-call 사용)
- `cli.html` → 삭제 (프론트엔드 채팅 컴포넌트로 대체)
- graph-tool-call sidecar → 불필요 (백엔드에 설치)
- 앱 크기 대폭 감소 (sidecar 11MB 제거)

## graph-tool-call 활용 비교

| 항목 | 현재 (데스크탑 앱) | 목표 (백엔드) |
|------|-------------------|---------------|
| 위치 | Tauri sidecar (PyInstaller) | xgen-workflow (pip install) |
| 호출 방식 | subprocess CLI | Python 라이브러리 직접 import |
| LLM 호출 | Rust에서 직접 SSE 파싱 | LangChain agent (자동) |
| tool_use 루프 | Rust 구현 (버그 다수) | LangGraph (검증됨) |
| 프로바이더 | Rust 멀티 프로바이더 | LangChain 프로바이더 (풍부) |

## 구현 순서

### Phase 1: 백엔드 (xgen-workflow)
1. `feat/ai-chatbot` 브랜치 생성
2. graph-tool-call 의존성 추가
3. AiChatService 구현 (gateway tools + LangChain agent)
4. SSE 스트리밍 엔드포인트 구현
5. 로컬 테스트

### Phase 2: 게이트웨이 (xgen-backend-gateway)
1. `feat/ai-chatbot` 브랜치 생성
2. /api/ai-chat/* 라우팅 추가
3. SSE 프록시 설정

### Phase 3: 프론트엔드 (xgen-frontend)
1. `feat/ai-chatbot` 브랜치 생성
2. AI 채팅 컴포넌트 구현
3. SSE 스트리밍 연동
4. 플로팅 버튼 + 채팅 패널 UI

### Phase 4: 데스크탑 앱 정리 (xgen-app)
1. Rust LLM 클라이언트 → 백엔드 API 호출로 전환
2. graph-tool-call sidecar 제거
3. cli.html 제거, 프론트엔드 채팅 컴포넌트 사용

## 일정 추정
- Phase 1: 2-3일
- Phase 2: 1일
- Phase 3: 3-5일
- Phase 4: 1일
