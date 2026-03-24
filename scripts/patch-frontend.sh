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

# 1. API Routes 제거 (output: 'export'에서 지원 안 됨)
API_DIR="$FRONTEND_DIR/src/app/api"
if [ -d "$API_DIR" ]; then
    echo "[PATCH] API Routes 제거: $API_DIR"
    rm -rf "$API_DIR"
    echo "[OK] API Routes 제거 완료"
else
    echo "[INFO] API Routes 이미 없음"
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

# 3. middleware.ts 제거 (output: 'export'에서 지원 안 됨)
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
