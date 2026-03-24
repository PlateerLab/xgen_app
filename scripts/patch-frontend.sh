#!/bin/bash

# xgen-frontend를 Tauri 정적 빌드(output: 'export')에 맞게 패치하는 스크립트
# - API Routes 제거 (Tauri에서는 백엔드 직접 호출)
# - platform.ts 누락 함수 추가 (index.ts re-export와 일치시킴)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$PROJECT_ROOT/frontend"

if [ ! -d "$FRONTEND_DIR" ]; then
    echo "[ERROR] frontend 디렉토리가 없습니다."
    exit 1
fi

echo "================================================"
echo "Tauri 빌드용 프론트엔드 패치"
echo "================================================"

# 1. 서버 전용 route handler 제거 (output: 'export'에서 지원 안 됨)
# API Routes (/api/*) 및 서버 전용 route.ts가 있는 디렉토리 제거
REMOVE_DIRS=(
    "$FRONTEND_DIR/src/app/api"    # API Routes
    "$FRONTEND_DIR/src/app/fe"     # 서버 파일 서빙 route
)

for DIR in "${REMOVE_DIRS[@]}"; do
    if [ -d "$DIR" ]; then
        echo "[PATCH] 서버 전용 디렉토리 제거: $DIR"
        rm -rf "$DIR"
        echo "[OK] 제거 완료"
    fi
done

# 혹시 다른 곳에 숨은 route.ts가 있으면 찾아서 경고
REMAINING_ROUTES=$(find "$FRONTEND_DIR/src/app" -name "route.ts" -o -name "route.js" 2>/dev/null || true)
if [ -n "$REMAINING_ROUTES" ]; then
    echo "[WARN] 추가 route handler 발견 (수동 확인 필요):"
    echo "$REMAINING_ROUTES"
fi

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

# 3. 동적 라우트에 generateStaticParams() 추가 (output: 'export' 필수)
echo "[PATCH] 동적 라우트 generateStaticParams 주입"
DYNAMIC_PAGES=$(find "$FRONTEND_DIR/src/app" -path '*\[*' -name "page.tsx" -o -path '*\[*' -name "page.ts" 2>/dev/null || true)
for PAGE in $DYNAMIC_PAGES; do
    if ! grep -q "generateStaticParams" "$PAGE"; then
        echo "  추가: $PAGE"
        TEMP_FILE=$(mktemp)
        # 'use client' directive가 있으면 그 뒤에 삽입, 없으면 맨 위에
        if head -5 "$PAGE" | grep -q "'use client'\|\"use client\""; then
            # 'use client' 줄까지 먼저 출력, 그 뒤에 generateStaticParams 추가
            USE_CLIENT_LINE=$(grep -n "use client" "$PAGE" | head -1 | cut -d: -f1)
            head -n "$USE_CLIENT_LINE" "$PAGE" > "$TEMP_FILE"
            cat >> "$TEMP_FILE" << 'GSP_EOF'

// === Tauri 빌드 패치: static export 호환 ===
export function generateStaticParams() {
  return [];
}
GSP_EOF
            tail -n +"$((USE_CLIENT_LINE + 1))" "$PAGE" >> "$TEMP_FILE"
        else
            cat > "$TEMP_FILE" << 'GSP_EOF'
// === Tauri 빌드 패치: static export 호환 ===
export function generateStaticParams() {
  return [];
}

GSP_EOF
            cat "$PAGE" >> "$TEMP_FILE"
        fi
        mv "$TEMP_FILE" "$PAGE"
    fi
done
echo "[OK] generateStaticParams 주입 완료"

# 4. middleware.ts 제거 (output: 'export'에서 지원 안 됨)
MIDDLEWARE_FILE="$FRONTEND_DIR/src/middleware.ts"
if [ -f "$MIDDLEWARE_FILE" ]; then
    echo "[PATCH] middleware.ts 제거 (static export 호환)"
    rm -f "$MIDDLEWARE_FILE"
    echo "[OK] middleware.ts 제거 완료"
fi

echo ""
echo "================================================"
echo "패치 완료!"
echo "================================================"
