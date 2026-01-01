# xgen_app 빌드 가이드

## 구조

프론트엔드 소스는 `xgen-frontend` Git 저장소에서 관리됩니다.
빌드 시 자동으로 최신 소스를 가져와서 Tauri 데스크톱 앱으로 빌드합니다.

```
xgen_app/
├── scripts/
│   ├── sync-frontend.sh    # 프론트엔드 소스 동기화
│   └── build.sh            # Tauri 앱 빌드 스크립트
├── src-tauri/              # Tauri 데스크톱 앱 설정
├── frontend/               # (자동 생성) xgen-frontend 소스
├── Dockerfile              # 웹 배포용 Docker 이미지
└── docker-compose.yml      # 웹 배포용 Docker Compose
```

## Tauri 데스크톱 앱 빌드

### 사전 요구사항

- Node.js 20+
- Rust (https://rustup.rs/)
- Tauri CLI: `cargo install tauri-cli`

### 빌드

```bash
# 데스크톱 앱 빌드 (xgen-frontend 최신 소스 자동 동기화)
./scripts/build.sh

# 개발 모드 실행 (핫리로드 지원)
./scripts/build.sh --dev

# 소스 동기화 건너뛰기 (이미 동기화된 경우)
./scripts/build.sh --skip-sync
```

### 빌드 결과물 위치

- macOS: `src-tauri/target/release/bundle/macos/XGEN.app`
- Windows: `src-tauri/target/release/XGEN.exe`
- Linux: `src-tauri/target/release/bundle/`

## 웹 배포 (Docker)

Tauri 앱 대신 웹으로 배포하려면:

```bash
# 프로덕션 이미지 빌드
docker-compose build

# 실행
docker-compose up -d
```

## 환경 변수

### 빌드 시

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `FRONTEND_BRANCH` | main | xgen-frontend 브랜치 |

### 런타임 (웹 배포)

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `NEXT_PUBLIC_BACKEND_HOST` | http://localhost | 백엔드 호스트 |
| `NEXT_PUBLIC_BACKEND_PORT` | 8000 | 백엔드 포트 |

## 특정 브랜치에서 빌드

```bash
# 로컬 빌드
FRONTEND_BRANCH=develop ./scripts/build.sh

# Docker 빌드
docker-compose build --build-arg FRONTEND_BRANCH=develop
```
