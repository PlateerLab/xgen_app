# API 추상화 레이어 설계

> 작성일: 2026-01-04
> 업무: xgen 플랫폼 API 추상화 레이어 (옵션 C)
> 구현 범위: **Phase 1 (core/ 인프라)**

## 목표

웹(xgen-frontend)과 데스크톱(xgen_app)에서 **동일한 API 인터페이스**를 사용하여 플랫폼 독립적인 코드 작성

## 현재 상태

| 환경 | 위치 | 통신 방식 | 인증 |
|------|------|----------|------|
| **웹** | `xgen-frontend/src/app/_common/api/` | fetch + SSE | 쿠키 기반 |
| **데스크톱** | `xgen_app/src/lib/tauri/` | Tauri IPC + Events | 로컬 |

## 설계 구조

```
xgen-frontend/src/app/_common/api/
├── core/                          # [신규] 추상화 핵심
│   ├── index.ts                   # 공개 API
│   ├── types.ts                   # 공통 타입
│   ├── ApiClient.interface.ts     # IApiClient 인터페이스
│   ├── WebApiClient.ts            # 웹 구현체
│   ├── TauriApiClient.ts          # Tauri 구현체
│   ├── platform.ts                # isTauri() 환경 감지
│   └── createApiClient.ts         # 팩토리 함수
│
├── domains/                       # [신규] 도메인별 래퍼
│   ├── llm.ts
│   ├── workflow.ts
│   └── ...
│
└── helper/apiClient.js            # [기존] 레거시 호환
```

## 핵심 인터페이스

```typescript
interface IApiClient {
  request<T>(endpoint: string, options?: RequestOptions): Promise<ApiResponse<T>>;
  get<T>(endpoint: string): Promise<ApiResponse<T>>;
  post<T>(endpoint: string, body?: unknown): Promise<ApiResponse<T>>;
  stream(endpoint: string, options: StreamRequestOptions): Promise<StreamCleanup>;
}

interface ILLMClient {  // Tauri 전용
  loadModel(options: LoadModelOptions): Promise<ModelStatus>;
  generate(prompt: string, options: GenerateOptions): Promise<StreamCleanup>;
  embedText(texts: string[]): Promise<number[][]>;
}
```

## 사용 예시

```typescript
import { createApiClient, isTauri } from '@/app/_common/api/core';
import { generateText } from '@/app/_common/api/domains/llm';

// 환경 자동 감지
const client = await createApiClient();

// 동일한 코드로 웹/데스크톱 모두 지원
await generateText('Hello', {
  onToken: (t) => console.log(t),
  onDone: () => console.log('완료'),
});
```

---

## 구현 작업 (Phase 1)

### 1. 타입 정의
**파일**: `xgen-frontend/src/app/_common/api/core/types.ts`
```typescript
- ApiResponse<T>, ApiError
- StreamCallbacks, StreamCleanup
- RequestOptions, StreamRequestOptions
```

### 2. 인터페이스 정의
**파일**: `xgen-frontend/src/app/_common/api/core/ApiClient.interface.ts`
```typescript
- IApiClient: request, get, post, put, delete, stream, upload
- ILLMClient: loadModel, generate, embedText (Tauri 전용)
```

### 3. 환경 감지
**파일**: `xgen-frontend/src/app/_common/api/core/platform.ts`
```typescript
- isTauri(): boolean
- getPlatform(): 'web' | 'tauri-standalone' | 'tauri-connected'
```

### 4. 웹 구현체
**파일**: `xgen-frontend/src/app/_common/api/core/WebApiClient.ts`
- 기존 `apiClient.js` 로직 포팅
- fetch 기반, 토큰 자동 갱신
- SSE 스트림 파싱

### 5. Tauri 구현체
**파일**: `xgen-frontend/src/app/_common/api/core/TauriApiClient.ts`
- `@tauri-apps/api` invoke/listen 사용
- Standalone/Connected 모드 분기
- 이벤트 기반 스트리밍

### 6. 팩토리 함수
**파일**: `xgen-frontend/src/app/_common/api/core/createApiClient.ts`
```typescript
- createApiClient(): Promise<IApiClient>
- createLLMClient(): Promise<ILLMClient | null>
```

### 7. 메인 Export
**파일**: `xgen-frontend/src/app/_common/api/core/index.ts`

## 생성할 파일 목록

| 파일 경로 | 설명 |
|----------|------|
| `xgen-frontend/src/app/_common/api/core/types.ts` | 공통 타입 |
| `xgen-frontend/src/app/_common/api/core/ApiClient.interface.ts` | 인터페이스 |
| `xgen-frontend/src/app/_common/api/core/platform.ts` | 환경 감지 |
| `xgen-frontend/src/app/_common/api/core/WebApiClient.ts` | 웹 구현체 |
| `xgen-frontend/src/app/_common/api/core/TauriApiClient.ts` | Tauri 구현체 |
| `xgen-frontend/src/app/_common/api/core/createApiClient.ts` | 팩토리 |
| `xgen-frontend/src/app/_common/api/core/index.ts` | Export |

---

## Phase 2: 레거시 호환 (다음 단계)

### 목표
기존 `apiClient.js`를 새 `WebApiClient`로 래핑하여 하위 호환성 유지

### 작업 내용
**파일**: `xgen-frontend/src/app/_common/api/helper/apiClient.js`

```javascript
// 기존 apiClient 함수를 새 클라이언트로 래핑
import { createApiClient } from '../core/createApiClient';

export const apiClient = async (url, options = {}, skipAuth = false) => {
    const client = await createApiClient();
    const response = await client.request(url, { ...options, skipAuth });

    // 기존 Response 형태로 반환 (하위 호환)
    return {
        ok: response.success,
        status: response.success ? 200 : 400,
        json: async () => response.data,
    };
};
```

### 체크리스트
- [ ] `apiClient.js` 수정
- [ ] `apiClientV2` 동일 래핑
- [ ] 기존 API 함수들 동작 테스트

---

## Phase 3: 도메인 API 이전 (다음 단계)

### 목표
기존 API 모듈을 TypeScript로 마이그레이션하고 새 클라이언트 사용

### 파일 구조
```
xgen-frontend/src/app/_common/api/domains/
├── llm.ts           # llmAPI.js → 이전
├── workflow.ts      # workflowAPI.js → 이전
├── config.ts        # configAPI.js → 이전
├── interaction.ts   # interactionAPI.js → 이전
├── auth.ts          # authAPI.js → 이전
└── rag/
    ├── document.ts
    ├── embedding.ts
    └── retrieval.ts
```

### 예시: domains/llm.ts
```typescript
import { createApiClient, createLLMClient } from '../core';
import { isTauri } from '../core/platform';

export async function getLLMStatus() {
  const client = await createApiClient();
  return client.get('/api/llm/status');
}

export async function generateText(prompt: string, options: GenerateOptions) {
  if (isTauri()) {
    const llmClient = await createLLMClient();
    if (llmClient) return llmClient.generate(prompt, options);
  }

  const client = await createApiClient();
  return client.stream('/api/llm/generate', {
    body: { prompt, ...options },
    callbacks: options,
  });
}
```

### 이전 순서
1. `llmAPI.js` → `domains/llm.ts`
2. `workflowAPI.js` → `domains/workflow.ts`
3. `configAPI.js` → `domains/config.ts`
4. 나머지 순차 이전

---

## Phase 4: Tauri 통합 (다음 단계)

### 목표
xgen_app 빌드 시 자동으로 API 추상화 레이어 포함

### 작업 1: 빌드 스크립트 수정
**파일**: `xgen_app/scripts/sync-frontend.sh`

```bash
# 기존 프론트엔드 동기화 후 추가
echo "[INFO] Tauri API 래퍼 확인..."

# core/ 폴더에 TauriApiClient가 있는지 확인
if [ -f "$FRONTEND_DIR/src/app/_common/api/core/TauriApiClient.ts" ]; then
    echo "[OK] API 추상화 레이어 존재"
else
    echo "[WARN] API 추상화 레이어 없음"
fi
```

### 작업 2: 기존 IPC 래퍼 통합
**파일**: `xgen_app/src/lib/tauri/` → `TauriApiClient.ts`로 통합

```typescript
// TauriApiClient가 기존 IPC 래퍼 기능 포함
// xgen_app/src/lib/tauri/ 코드를 TauriApiClient로 병합
```

### 작업 3: package.json 의존성
**파일**: `xgen-frontend/package.json`

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.0.0"  // optional peer dependency
  },
  "optionalDependencies": {
    "@tauri-apps/api": "^2.0.0"
  }
}
```

### 작업 4: Next.js 설정
**파일**: `xgen-frontend/next.config.ts`

```typescript
// 웹 빌드 시 Tauri 모듈 제외
webpack: (config, { isServer }) => {
  if (!isServer) {
    config.resolve.alias['@tauri-apps/api'] = false;
  }
  return config;
}
```

---

## 전체 일정 요약

| Phase | 범위 | 상태 |
|-------|------|------|
| **Phase 1** | core/ 인프라 구축 | 🔴 **현재 진행** |
| Phase 2 | 레거시 apiClient.js 래핑 | ⚪ 대기 |
| Phase 3 | 도메인 API 이전 | ⚪ 대기 |
| Phase 4 | Tauri 빌드 통합 | ⚪ 대기 |

---

## 참고 파일

- `xgen-frontend/src/app/_common/api/helper/apiClient.js` - 기존 웹 클라이언트
- `xgen_app/src/lib/tauri/llm.ts` - Tauri IPC 패턴
- `xgen-frontend/src/app/_common/api/workflow/workflowAPI.js` - SSE 스트리밍 참조
