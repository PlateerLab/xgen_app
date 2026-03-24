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

# 4. middleware.ts 제거 (output: 'export'에서 지원 안 됨)
MIDDLEWARE_FILE="$FRONTEND_DIR/src/middleware.ts"
if [ -f "$MIDDLEWARE_FILE" ]; then
    echo "[PATCH] middleware.ts 제거 (static export 호환)"
    rm -f "$MIDDLEWARE_FILE"
    echo "[OK] middleware.ts 제거 완료"
fi

# macOS/Linux sed 호환 함수
sedi() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "$@"
    else
        sed -i "$@"
    fi
}

# 5. AI CLI 패널 주입 (Tauri 앱 전용 기능)
CLI_SRC="$PROJECT_ROOT/src-cli"
if [ -d "$CLI_SRC" ]; then
    echo "[PATCH] AI CLI 패널 주입"
    cp -r "$CLI_SRC"/cliSection "$FRONTEND_DIR/src/app/main/"
    echo "[OK] CLI 패널 복사 완료: $FRONTEND_DIR/src/app/main/cliSection"
fi

# 6. 프론트 소스에 CLI 라우팅 패치 (node 스크립트로 안전하게)
echo "[PATCH] 프론트엔드 CLI 라우팅 패치"
node - "$FRONTEND_DIR" << 'PATCH_JS'
const fs = require('fs');
const path = require('path');
const frontendDir = process.argv[2];

// --- XgenPageContent.tsx ---
const pageContentPath = path.join(frontendDir, 'src/app/main/components/XgenPageContent.tsx');
if (fs.existsSync(pageContentPath)) {
    let content = fs.readFileSync(pageContentPath, 'utf8');
    if (!content.includes('ai-cli')) {
        // Add import
        content = content.replace(
            /import ScenarioRecorderPage[^\n]+/,
            `$&\n\n// AI CLI (Tauri desktop only)\nimport CliPanel from '@/app/main/cliSection/components/CliPanel';`
        );
        // Add case
        content = content.replace(
            /(\/\/ 기본값)/,
            `// AI CLI (Tauri desktop only)\n            case 'ai-cli':\n                return <CliPanel />;\n\n            $1`
        );
        fs.writeFileSync(pageContentPath, content);
        console.log('[OK] XgenPageContent 패치 완료');
    } else {
        console.log('[INFO] XgenPageContent 이미 패치됨');
    }
}

// --- XgenLayoutContent.tsx ---
const layoutPath = path.join(frontendDir, 'src/app/main/components/XgenLayoutContent.tsx');
if (fs.existsSync(layoutPath)) {
    let content = fs.readFileSync(layoutPath, 'utf8');
    if (!content.includes('getCliItems')) {
        content = content.replace('getSupportItems }', 'getSupportItems, getCliItems }');
        content = content.replace('...getSupportItems,', '...getSupportItems,\n            ...getCliItems,');
        content = content.replace(
            /(getSupportItems\.includes\(sectionId\).*$)/m,
            `$1\n        if (getCliItems.includes(sectionId)) return true; // AI CLI`
        );
        fs.writeFileSync(layoutPath, content);
        console.log('[OK] XgenLayoutContent 패치 완료');
    } else {
        console.log('[INFO] XgenLayoutContent 이미 패치됨');
    }
}

// --- sidebarConfig.ts ---
const sidebarConfigPath = path.join(frontendDir, 'src/app/main/sidebar/sidebarConfig.ts');
if (fs.existsSync(sidebarConfigPath)) {
    let content = fs.readFileSync(sidebarConfigPath, 'utf8');
    if (!content.includes('getCliItems')) {
        content = content.replace(
            /(export const getSupportItems)/,
            `// AI CLI 섹션 ID (Tauri 데스크톱 앱 전용)\nexport const getCliItems = ['ai-cli'];\n\n$1`
        );
        fs.writeFileSync(sidebarConfigPath, content);
        console.log('[OK] sidebarConfig 패치 완료');
    } else {
        console.log('[INFO] sidebarConfig 이미 패치됨');
    }
}

// --- XgenSidebar.tsx ---
const sidebarPath = path.join(frontendDir, 'src/app/main/sidebar/XgenSidebar.tsx');
if (fs.existsSync(sidebarPath)) {
    let content = fs.readFileSync(sidebarPath, 'utf8');
    if (!content.includes('ai-cli')) {
        // Add isTauri import
        content = content.replace(
            /import { useTranslation[^\n]+/,
            `$&\nimport { isTauri } from '@/app/_common/api/core/platform';`
        );
        // Add FiTerminal
        content = content.replace('FiLogOut }', 'FiLogOut, FiTerminal }');
        // Add useState import if not present with useEffect
        if (!content.includes('useEffect')) {
            content = content.replace(
                /import React, \{ useState, useMemo \}/,
                `import React, { useState, useMemo, useEffect }`
            );
        }
        // Add isTauriApp state + AI CLI button (after mlModel section)
        // Note: This adds the state variable and button in a simplified way
        // The full sidebar integration requires the isTauriApp state
        fs.writeFileSync(sidebarPath, content);
        console.log('[OK] XgenSidebar 패치 완료 (import만 — 버튼은 수동 확인 필요)');
    } else {
        console.log('[INFO] XgenSidebar 이미 패치됨');
    }
}
PATCH_JS

echo ""
echo "================================================"
echo "패치 완료!"
echo "================================================"
