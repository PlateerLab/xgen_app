#!/bin/bash

# xgen-frontend를 Tauri 정적 빌드(output: 'export')에 맞게 패치하는 스크립트
# - API Routes 제거 (Tauri에서는 백엔드 직접 호출)
# - platform.ts 누락 함수 추가 (index.ts re-export와 일치시킴)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="${FRONTEND_DIR:-$PROJECT_ROOT/frontend}"

if [ ! -d "$FRONTEND_DIR" ]; then
    echo "[ERROR] frontend 디렉토리가 없습니다."
    exit 1
fi

echo "================================================"
echo "Tauri 빌드용 프론트엔드 패치"
echo "================================================"

# 1. 서버 전용 route handler 제거 (output: 'export'에서 지원 안 됨)
# route.ts가 있는 디렉토리만 제거 (일반 TS 모듈은 보존)
echo "[PATCH] 서버 전용 route handler 제거"
find "$FRONTEND_DIR/src/app" -name "route.ts" -o -name "route.js" 2>/dev/null | while read ROUTE_FILE; do
    ROUTE_DIR=$(dirname "$ROUTE_FILE")
    echo "  제거: $ROUTE_DIR"
    rm -rf "$ROUTE_DIR"
done
echo "[OK] route handler 제거 완료"

# /fe 디렉토리 제거 (서버 파일 서빙)
if [ -d "$FRONTEND_DIR/src/app/fe" ]; then
    echo "[PATCH] /fe 디렉토리 제거"
    rm -rf "$FRONTEND_DIR/src/app/fe"
    echo "[OK] 제거 완료"
fi

# 빈 디렉토리 정리
find "$FRONTEND_DIR/src/app/api" -type d -empty -delete 2>/dev/null || true

# 2. platform.ts 누락 함수 추가
PLATFORM_FILE="$FRONTEND_DIR/src/app/_common/api/core/platform.ts"
if [ -f "$PLATFORM_FILE" ]; then
    # getPlatform이 없으면 추가
    if ! grep -q "getPlatform" "$PLATFORM_FILE"; then
        echo "[PATCH] platform.ts에 누락 함수 추가"
        cat >> "$PLATFORM_FILE" << 'PATCH_EOF'

// === Tauri 빌드 패치: 누락된 export 추가 ===

/**
 * 플랫폼 타입
 */
export type PlatformType = 'tauri' | 'web' | 'server';

/**
 * 현재 플랫폼 감지 (async)
 */
export async function getPlatform(): Promise<PlatformType> {
  if (isServer()) return 'server';
  if (isTauri()) return 'tauri';
  return 'web';
}

/**
 * 현재 플랫폼 감지 (sync)
 */
export function getPlatformSync(): PlatformType {
  if (isServer()) return 'server';
  if (isTauri()) return 'tauri';
  return 'web';
}

/**
 * Tauri Core API 가져오기
 */
export async function getTauriCoreApi() {
  if (!isTauri()) return null;
  try {
    return await import('@tauri-apps/api/core');
  } catch {
    return null;
  }
}

/**
 * Tauri Event API 가져오기
 */
export async function getTauriEventApi() {
  if (!isTauri()) return null;
  try {
    return await import('@tauri-apps/api/event');
  } catch {
    return null;
  }
}
PATCH_EOF
        echo "[OK] platform.ts 패치 완료"
    else
        echo "[INFO] platform.ts 이미 패치됨"
    fi
else
    echo "[WARN] platform.ts 없음 - 패치 건너뜀"
fi

# 3. 동적 라우트 디렉토리 제거 (output: 'export' + 'use client' 공존 불가)
# [param] 형태의 동적 경로는 static export에서 generateStaticParams 필요하지만
# 'use client' 컴포넌트와 함께 쓸 수 없으므로 해당 페이지 자체를 제거
echo "[PATCH] 동적 라우트 디렉토리 제거"
DYNAMIC_DIRS=$(find "$FRONTEND_DIR/src/app" -type d -name '\[*' 2>/dev/null | sort -r || true)
for DIR in $DYNAMIC_DIRS; do
    # 부모 디렉토리도 동적이면(중첩) 자식만 제거하면 됨 (sort -r로 깊은 것 먼저)
    if [ -d "$DIR" ]; then
        echo "  제거: $DIR"
        rm -rf "$DIR"
    fi
done
# 빈 부모 디렉토리 정리
find "$FRONTEND_DIR/src/app" -type d -empty -delete 2>/dev/null || true
echo "[OK] 동적 라우트 제거 완료"

# 4. next.config.ts 패치 — images.unoptimized 추가 (static export에서 next/image 사용 위해 필수)
NEXT_CONFIG="$FRONTEND_DIR/next.config.ts"
if [ -f "$NEXT_CONFIG" ] && ! grep -q "unoptimized" "$NEXT_CONFIG"; then
    echo "[PATCH] next.config.ts — images.unoptimized: true 추가"
    node - "$NEXT_CONFIG" << 'NEXTCONFIG_PATCH'
const fs = require('fs');
const configPath = process.argv[2];
let content = fs.readFileSync(configPath, 'utf8');

// Add images: { unoptimized: true } for static export (next/image requirement)
if (!content.includes('unoptimized')) {
    content = content.replace(
        /(const nextConfig:\s*NextConfig\s*=\s*\{)/,
        `$1\n    images: { unoptimized: true },`
    );
    fs.writeFileSync(configPath, content);
    console.log('[OK] next.config.ts images.unoptimized 패치 완료');
} else {
    console.log('[INFO] next.config.ts 이미 패치됨');
}
NEXTCONFIG_PATCH
fi

# 5. middleware.ts 제거 (output: 'export'에서 지원 안 됨)
MIDDLEWARE_FILE="$FRONTEND_DIR/src/middleware.ts"
if [ -f "$MIDDLEWARE_FILE" ]; then
    echo "[PATCH] middleware.ts 제거 (static export 호환)"
    rm -f "$MIDDLEWARE_FILE"
    echo "[OK] middleware.ts 제거 완료"
fi

# 5. config.js 패치 — Tauri 정적 빌드에서는 클라이언트도 직접 백엔드 URL 사용
CONFIG_FILE="$FRONTEND_DIR/src/app/config.js"
if [ -f "$CONFIG_FILE" ] && ! grep -q "isTauriStaticExport" "$CONFIG_FILE"; then
    echo "[PATCH] config.js — Tauri 정적 빌드용 BASE_URL 설정"
    node - "$CONFIG_FILE" << 'CONFIG_PATCH'
const fs = require('fs');
const configPath = process.argv[2];
let content = fs.readFileSync(configPath, 'utf8');

// Replace the client-side BASE_URL logic to use actual backend URL in Tauri
content = content.replace(
    /} else \{\n\s*\/\/ 클라이언트 사이드:.*\n\s*BASE_URL = '';/,
    `} else {
    // 클라이언트 사이드: Tauri 정적 빌드에서는 Next.js 프록시가 없으므로 직접 URL 사용
    // isTauriStaticExport flag for patch detection
    const isTauriStaticExport = typeof window !== 'undefined' && window.__TAURI_INTERNALS__;
    if (isTauriStaticExport) {
        const hasPortInHostClient = /:\\d+$/.test(host_url.replace(/\\/$/, ''));
        if (!port || hasPortInHostClient) {
            BASE_URL = host_url.replace(/\\/$/, '');
        } else {
            BASE_URL = host_url.replace(/\\/$/, '') + ':' + port;
        }
    } else {
        BASE_URL = '';
    }`
);

fs.writeFileSync(configPath, content);
console.log('[OK] config.js 패치 완료');
CONFIG_PATCH
fi

# macOS/Linux sed 호환 함수
sedi() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "$@"
    else
        sed -i "$@"
    fi
}

# 5. AI CLI — 별도 윈도우 방식 (cli.html을 빌드 출력에 복사)
CLI_HTML="$PROJECT_ROOT/src-cli/cli.html"
if [ -f "$CLI_HTML" ]; then
    echo "[PATCH] AI CLI 윈도우 파일 준비"
    # cli.html은 빌드 후 frontend/out/에 복사됨 (아래 post-build에서)
    echo "[OK] cli.html 준비 완료"
fi

# 6. 사이드바에 AI CLI 버튼 추가 (클릭 시 별도 윈도우 열기)
echo "[PATCH] 사이드바 AI CLI 버튼 패치"
node "$SCRIPT_DIR/patch-sidebar-cli.js" "$FRONTEND_DIR"

echo ""
echo "================================================"
echo "패치 완료!"
echo "================================================"
