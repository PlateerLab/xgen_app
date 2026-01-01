# XGEN App

XGEN 프론트엔드를 Tauri 데스크톱 앱 또는 Docker 웹 서비스로 배포하는 프로젝트입니다.

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
