#!/usr/bin/env node
/**
 * apiClient.js의 getToken()을 패치하여
 * Tauri WKWebView에서 document.cookie 접근 불가 시
 * Rust 세션(cli_get_token)에서 토큰을 가져오도록 한다.
 *
 * 또한 로그인 성공 시 Rust 세션에도 토큰을 저장하도록
 * CookieProvider를 패치한다.
 */
const fs = require('fs');
const path = require('path');

const frontendDir = process.argv[2];
if (!frontendDir) {
    console.error('Usage: node patch-api-auth.js <frontend-dir>');
    process.exit(1);
}

// ============================================================
// Part 1: apiClient.js — getToken() 패치
// ============================================================

const apiClientPath = path.join(frontendDir, 'src/app/_common/api/helper/apiClient.js');
if (!fs.existsSync(apiClientPath)) {
    console.log('[WARN] apiClient.js not found');
    process.exit(0);
}

let content = fs.readFileSync(apiClientPath, 'utf8');

if (content.includes('tauriTokenCache')) {
    console.log('[INFO] apiClient.js already patched');
} else {
    console.log('[PATCH] apiClient.js — getToken() Tauri fallback 추가');

    // getToken 함수를 확장
    content = content.replace(
        /const getToken = \(\) => \{\s*\n\s*return getAuthCookie\('access_token'\);\s*\n\};/,
        `// Tauri 토큰 캐시 (Rust 세션에서 가져온 토큰)
let tauriTokenCache = null;

/**
 * 인증 토큰 가져오기
 * 1. 쿠키 (document.cookie)
 * 2. Tauri 토큰 캐시 (Rust 세션에서 미리 가져온 값)
 * 3. localStorage fallback
 */
const getToken = () => {
    // 1. 쿠키에서 시도
    const cookieToken = getAuthCookie('access_token');
    if (cookieToken) return cookieToken;

    // 2. Tauri 캐시 (initTauriToken으로 미리 로드)
    if (tauriTokenCache) return tauriTokenCache;

    // 3. localStorage fallback (Tauri WKWebView에서 cookie 불가 시)
    try {
        const stored = localStorage.getItem('xgen_access_token');
        if (stored) return stored;
    } catch {}

    return null;
};

/**
 * Tauri 환경에서 Rust 세션의 토큰을 미리 로드
 */
const initTauriToken = async () => {
    if (typeof window === 'undefined') return;
    if (!window.__TAURI_INTERNALS__) return;
    try {
        const { invoke } = await import('@tauri-apps/api/core');
        const token = await invoke('cli_get_token');
        if (token) {
            tauriTokenCache = token;
            // localStorage에도 저장 (다른 코드에서도 접근 가능)
            try { localStorage.setItem('xgen_access_token', token); } catch {}
        }
    } catch {}
};

// 앱 시작 시 Tauri 토큰 로드
initTauriToken();`
    );

    // setCookieAuth 호출 후 localStorage에도 저장하도록 패치
    // 로그인 성공 시 토큰이 쿠키에 저장되면 localStorage에도 동기화
    if (!content.includes('xgen_access_token')) {
        // 이미 위에서 추가했으므로 패스
    }

    fs.writeFileSync(apiClientPath, content);
    console.log('[OK] apiClient.js patched');
}

// ============================================================
// Part 2: CookieProvider — 로그인 시 Rust에도 토큰 전달
// ============================================================

const cookieProviderPath = path.join(frontendDir, 'src/app/_common/components/CookieProvider.tsx');
if (!fs.existsSync(cookieProviderPath)) {
    console.log('[WARN] CookieProvider.tsx not found');
    process.exit(0);
}

let cpContent = fs.readFileSync(cookieProviderPath, 'utf8');

if (cpContent.includes('syncTokenToTauri')) {
    console.log('[INFO] CookieProvider already patched');
} else {
    console.log('[PATCH] CookieProvider — 로그인 시 Tauri/localStorage에 토큰 동기화');

    // setUser 호출 근처에 Tauri 토큰 동기화 추가
    // setCookieAuth('access_token', userData.access_token) 뒤에 추가
    cpContent = cpContent.replace(
        /setCookieAuth\('access_token', userData\.access_token\);/,
        `setCookieAuth('access_token', userData.access_token);

                // Tauri + localStorage 토큰 동기화
                const syncTokenToTauri = async (token: string) => {
                    try { localStorage.setItem('xgen_access_token', token); } catch {}
                    if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
                        try {
                            const { invoke } = await import('@tauri-apps/api/core');
                            await invoke('cli_set_token', { token });
                        } catch {}
                    }
                };
                syncTokenToTauri(userData.access_token);`
    );

    fs.writeFileSync(cookieProviderPath, cpContent);
    console.log('[OK] CookieProvider patched');
}
