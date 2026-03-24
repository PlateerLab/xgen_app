#!/bin/bash

# xgen-frontend 소스를 동기화하는 스크립트
# 빌드 전에 실행하여 최신 프론트엔드 소스를 가져옴

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
# GitLab은 반드시 https:// 사용 (http://는 인증 실패)
# CI에서는 GITLAB_TOKEN 환경변수, 로컬에서는 ~/.claude/.secrets 참조
if [ -z "$GITLAB_TOKEN" ] && [ -f "$HOME/.claude/.secrets" ]; then
    source "$HOME/.claude/.secrets"
fi

if [ -n "$GITLAB_TOKEN" ]; then
    FRONTEND_REPO="https://sonsj97:${GITLAB_TOKEN}@gitlab.x2bee.com/xgen2.0/xgen-frontend.git"
else
    FRONTEND_REPO="https://gitlab.x2bee.com/xgen2.0/xgen-frontend.git"
fi
BRANCH="${FRONTEND_BRANCH:-main}"

echo "================================================"
echo "xgen-frontend 소스 동기화"
echo "================================================"
echo "Project Root: $PROJECT_ROOT"
echo "Frontend Dir: $FRONTEND_DIR"
echo "Branch: $BRANCH"
echo ""

# frontend 디렉토리가 존재하는지 확인
if [ -d "$FRONTEND_DIR" ]; then
    echo "[INFO] 기존 frontend 디렉토리 발견. 최신 소스로 업데이트..."
    cd "$FRONTEND_DIR"

    # git 저장소인지 확인
    if [ -d ".git" ]; then
        echo "[INFO] Git pull 실행..."
        git fetch origin
        git checkout "$BRANCH"
        git pull origin "$BRANCH"
    else
        echo "[WARN] .git 디렉토리 없음. 디렉토리 삭제 후 다시 클론..."
        cd "$PROJECT_ROOT"
        rm -rf "$FRONTEND_DIR"
        git clone --depth 1 --branch "$BRANCH" "$FRONTEND_REPO" "$FRONTEND_DIR"
    fi
else
    echo "[INFO] frontend 디렉토리 없음. 새로 클론..."
    git clone --depth 1 --branch "$BRANCH" "$FRONTEND_REPO" "$FRONTEND_DIR"
fi

cd "$FRONTEND_DIR"
COMMIT_HASH=$(git rev-parse --short HEAD)
COMMIT_DATE=$(git log -1 --format=%ci)

echo ""
echo "================================================"
echo "동기화 완료!"
echo "================================================"
echo "Commit: $COMMIT_HASH"
echo "Date: $COMMIT_DATE"
echo ""

# API 추상화 레이어 확인
echo "================================================"
echo "API 추상화 레이어 확인"
echo "================================================"

API_CORE_DIR="$FRONTEND_DIR/src/app/_common/api/core"
API_DOMAINS_DIR="$FRONTEND_DIR/src/app/_common/api/domains"

if [ -f "$API_CORE_DIR/TauriApiClient.ts" ]; then
    echo "[OK] TauriApiClient.ts 존재"
else
    echo "[WARN] TauriApiClient.ts 없음 - Tauri IPC 지원 불가"
fi

if [ -f "$API_CORE_DIR/createApiClient.ts" ]; then
    echo "[OK] createApiClient.ts 존재"
else
    echo "[WARN] createApiClient.ts 없음 - API 팩토리 없음"
fi

if [ -d "$API_DOMAINS_DIR" ]; then
    echo "[OK] domains/ 폴더 존재"
else
    echo "[INFO] domains/ 폴더 없음 - 레거시 API 사용"
fi

echo ""
