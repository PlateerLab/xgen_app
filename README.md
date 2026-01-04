# XGEN App

XGEN 프론트엔드를 Tauri 데스크톱 앱 또는 Docker 웹 서비스로 배포하는 프로젝트입니다.

---

## 🎯 사용 모드 안내

XGEN은 **배포 방식**에 따라 사용 가능한 모드가 다릅니다:

| 배포 방식 | 로컬 모드 | 서버 모드 |
|----------|:--------:|:--------:|
| 🖥️ **데스크톱 앱** (Tauri) | ✅ | ✅ |
| 🌐 **웹 서비스** (Docker) | ❌ | ✅ |

> 💡 **로컬 모드**는 데스크톱 앱에서만 사용 가능합니다. 웹 버전은 항상 서버에 연결됩니다.

### 🏠 로컬 모드 (Standalone) - 데스크톱 앱 전용

**인터넷 없이 내 컴퓨터에서 AI 실행** *(Tauri 앱에서만 사용 가능)*

```
┌─────────────┐     ┌─────────────┐
│   XGEN 앱   │ ──▶ │  내 컴퓨터   │
│   (화면)    │     │  (AI 모델)  │
└─────────────┘     └─────────────┘
```

| 장점 | 단점 |
|------|------|
| ✅ 인터넷 불필요 | ⚠️ 컴퓨터 성능 필요 (GPU 권장) |
| ✅ 데이터가 내 PC에만 저장 | ⚠️ 일부 기능 제한 |
| ✅ 빠른 응답 속도 | |

**적합한 사용자:**
- 보안이 중요한 업무
- 인터넷이 불안정한 환경
- 개인 프라이버시가 중요한 경우

---

### 🌐 서버 모드 (Connected)

**서버에 연결하여 모든 기능 사용**

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   XGEN 앱   │ ──▶ │   Gateway   │ ──▶ │  AI 서버들   │
│   (화면)    │     │   서버      │     │  (클라우드)  │
└─────────────┘     └─────────────┘     └─────────────┘
```

| 장점 | 단점 |
|------|------|
| ✅ 모든 기능 사용 가능 | ⚠️ 인터넷 필요 |
| ✅ 강력한 AI 모델 사용 | ⚠️ 서버 비용 발생 가능 |
| ✅ 파일 업로드, RAG 검색 등 | |

**적합한 사용자:**
- 팀으로 협업하는 경우
- 고성능 AI가 필요한 경우
- 워크플로우, 문서 검색 기능 사용

---

### 🔄 모드 전환 방법 (데스크톱 앱 전용)

> ℹ️ 웹 버전은 자동으로 서버 모드로 동작하며, 모드 전환이 필요 없습니다.

#### 방법 1: 앱 설정 화면에서 변경 (TODO)

> ⚠️ **현재 개발 중**: 설정 UI에서 모드 전환 기능이 추가될 예정입니다.

완성되면 아래와 같이 사용할 수 있습니다:
```
┌────────────────────────────────────────────────────┐
│  XGEN                    [🔌 로컬 모드 ▼]  [설정]  │
│                              ↑                     │
│                         여기를 클릭!               │
└────────────────────────────────────────────────────┘
```

#### 방법 2: 개발자 도구 콘솔 사용 (고급)
`F12` 또는 `Ctrl+Shift+I`로 개발자 도구를 열고 콘솔에서 입력:

```javascript
// 현재 모드 확인
await window.__TAURI__.core.invoke('get_app_mode')

// 서버 모드로 전환
await window.__TAURI__.core.invoke('set_app_mode', {
  mode: 'connected',
  serverUrl: 'http://localhost:8000'
})

// 로컬 모드로 전환
await window.__TAURI__.core.invoke('set_app_mode', {
  mode: 'standalone',
  serverUrl: null
})
```

**서버 주소 예시:**
- 로컬 Docker: `http://localhost:8000`
- 회사 서버: `http://xgen.company.com:8000`

> 💡 **팁**: 서버 모드 전환 전에 Docker 서비스가 실행 중인지 확인하세요!

---

### 📊 기능 비교표

| 기능 | 로컬 모드 | 서버 모드 |
|------|:--------:|:--------:|
| AI 채팅 | ✅ | ✅ |
| 로컬 모델 사용 | ✅ | ❌ |
| 클라우드 모델 (GPT 등) | ❌ | ✅ |
| 워크플로우 실행 | ❌ | ✅ |
| 문서 업로드 & RAG | ❌ | ✅ |
| MCP 도구 | ✅ | ✅ |
| 오프라인 사용 | ✅ | ❌ |

---

## 프로젝트 구조

```
xgen_app/
├── scripts/
│   ├── sync-frontend.sh    # 프론트엔드 소스 동기화
│   └── build.sh            # 빌드 스크립트
├── src-tauri/              # Tauri 데스크톱 앱 설정
├── frontend/               # (빌드 시 자동 생성) xgen-frontend 소스
├── Dockerfile              # 웹 배포용 이미지
└── docker-compose.yml      # 웹 배포용 Compose
```

## 설치

### 사전 요구사항

**데스크톱 앱 빌드 시:**
- Node.js 20+
- Rust ([rustup.rs](https://rustup.rs/))
- Tauri CLI: `cargo install tauri-cli`

**웹 배포 시:**
- Docker & Docker Compose

## 빌드

### 데스크톱 앱 (Tauri)

```bash
# 프로덕션 빌드
./scripts/build.sh

# 개발 모드 (핫리로드)
./scripts/build.sh --dev

# 소스 동기화 건너뛰기
./scripts/build.sh --skip-sync
```

**빌드 결과물:**
- macOS: `src-tauri/target/release/bundle/macos/XGEN.app`
- Windows: `src-tauri/target/release/XGEN.exe`
- Linux: `src-tauri/target/release/bundle/`

### 웹 서비스 (Docker)

```bash
# 이미지 빌드
docker-compose build

# 실행
docker-compose up -d

# 개발 모드 (로컬 소스 마운트)
docker-compose --profile dev up xgen-frontend-dev
```

## 배포

### 환경 변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `FRONTEND_BRANCH` | main | 빌드할 xgen-frontend 브랜치 |
| `NEXT_PUBLIC_BACKEND_HOST` | http://localhost | 백엔드 API 호스트 |
| `NEXT_PUBLIC_BACKEND_PORT` | 8000 | 백엔드 API 포트 |

### 특정 브랜치 빌드

```bash
# 로컬 빌드
FRONTEND_BRANCH=develop ./scripts/build.sh

# Docker 빌드
docker-compose build --build-arg FRONTEND_BRANCH=develop
```

### 프로덕션 배포

```bash
# 환경 변수 설정 후 실행
NEXT_PUBLIC_BACKEND_HOST=https://api.example.com \
NEXT_PUBLIC_BACKEND_PORT=443 \
docker-compose up -d
```

## 포트

- **3000**: 프론트엔드 웹 서비스
