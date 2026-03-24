#!/bin/bash

# xgen_app Tauri 데스크톱 앱 빌드 스크립트
# 1. xgen-frontend 소스 동기화
# 2. 프론트엔드 의존성 설치
# 3. Tauri 앱 빌드

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$PROJECT_ROOT/frontend"

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_step() {
    echo ""
    echo -e "${GREEN}==== $1 ====${NC}"
    echo ""
}

print_warn() {
    echo -e "${YELLOW}[WARN] $1${NC}"
}

print_error() {
    echo -e "${RED}[ERROR] $1${NC}"
}

# 파라미터 파싱
SKIP_SYNC=false
DEV_MODE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-sync)
            SKIP_SYNC=true
            shift
            ;;
        --dev)
            DEV_MODE=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --skip-sync  프론트엔드 소스 동기화 건너뛰기"
            echo "  --dev        개발 모드로 실행 (Tauri dev 서버)"
            echo "  -h, --help   도움말 표시"
            exit 0
            ;;
        *)
            print_error "알 수 없는 옵션: $1"
            exit 1
            ;;
    esac
done

cd "$PROJECT_ROOT"

# Step 1: 프론트엔드 소스 동기화
if [ "$SKIP_SYNC" = false ]; then
    print_step "Step 1: xgen-frontend 소스 동기화"
    bash "$SCRIPT_DIR/sync-frontend.sh"
else
    print_warn "프론트엔드 소스 동기화 건너뜀 (--skip-sync)"
fi

# frontend 디렉토리 확인
if [ ! -d "$FRONTEND_DIR" ]; then
    print_error "frontend 디렉토리가 없습니다. --skip-sync 옵션을 제거하고 다시 실행하세요."
    exit 1
fi

# Step 2: Tauri 정적 빌드용 패치 (API Routes 제거, 누락 함수 추가)
print_step "Step 2: Tauri 빌드 패치"
bash "$SCRIPT_DIR/patch-frontend.sh"

# Step 3: 프론트엔드 의존성 설치
print_step "Step 3: 프론트엔드 의존성 설치"
cd "$FRONTEND_DIR"
npm install

# Step 4: 프론트엔드 정적 빌드 (Tauri 번들용)
print_step "Step 4: 프론트엔드 정적 빌드"
export TAURI_ENV=true
export NEXT_PUBLIC_BACKEND_HOST="${NEXT_PUBLIC_BACKEND_HOST:-https://xgen.x2bee.com}"
export NEXT_PUBLIC_BACKEND_PORT="${NEXT_PUBLIC_BACKEND_PORT:-}"
echo "Backend: $NEXT_PUBLIC_BACKEND_HOST:$NEXT_PUBLIC_BACKEND_PORT"
npm run build

if [ ! -d "$FRONTEND_DIR/out" ]; then
    print_error "프론트엔드 빌드 실패: out/ 디렉토리가 생성되지 않았습니다."
    exit 1
fi
echo "프론트엔드 빌드 완료: $(find "$FRONTEND_DIR/out" -type f | wc -l) files"

# Step 5: Tauri 실행/빌드
cd "$PROJECT_ROOT"

if [ "$DEV_MODE" = true ]; then
    print_step "Step 5: Tauri 개발 모드 실행"
    cd src-tauri
    cargo tauri dev
else
    print_step "Step 5: Tauri 앱 빌드"
    cd src-tauri
    cargo tauri build

    print_step "Step 6: 빌드 완료!"
    echo "빌드된 앱 위치:"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "  - macOS: $PROJECT_ROOT/src-tauri/target/release/bundle/macos/"
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "  - Linux: $PROJECT_ROOT/src-tauri/target/release/bundle/"
    else
        echo "  - Windows: $PROJECT_ROOT/src-tauri/target/release/"
    fi
fi
