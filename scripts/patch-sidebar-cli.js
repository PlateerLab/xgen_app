#!/usr/bin/env node
/**
 * XgenSidebar.tsx에 AI CLI 버튼을 패치하는 스크립트
 * 사용법: node patch-sidebar-cli.js <frontend-dir>
 *
 * 현재 XgenSidebar는 SidebarLayout에 props를 전달하는 구조.
 * afterSections에 AI CLI 버튼을 추가한다.
 */
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const frontendDir = process.argv[2];
if (!frontendDir) {
    console.error('Usage: node patch-sidebar-cli.js <frontend-dir>');
    process.exit(1);
}

// XgenSidebar.tsx 경로 탐색 (구조 변경 대응)
const candidates = [
    'src/app/main/components/layout/XgenSidebar.tsx',
    'src/app/main/sidebar/XgenSidebar.tsx',
];

let sidebarPath = null;
for (const candidate of candidates) {
    const fullPath = path.join(frontendDir, candidate);
    if (fs.existsSync(fullPath)) {
        sidebarPath = fullPath;
        break;
    }
}

if (!sidebarPath) {
    // fallback: find로 검색
    try {
        const result = execSync(`find "${frontendDir}/src" -name "XgenSidebar.tsx" -type f`, { encoding: 'utf8' }).trim();
        if (result) sidebarPath = result.split('\n')[0];
    } catch {}
}

if (!sidebarPath) {
    console.log('[WARN] XgenSidebar.tsx not found — skipping AI CLI patch');
    process.exit(0);
}

console.log(`[INFO] Found XgenSidebar at: ${sidebarPath}`);

let content = fs.readFileSync(sidebarPath, 'utf8');
if (content.includes('open_cli_window') || content.includes('openCliWindow')) {
    console.log('[INFO] XgenSidebar 이미 패치됨');
    process.exit(0);
}

console.log('[PATCH] XgenSidebar에 AI CLI 버튼 추가...');

// Detect which sidebar structure we're dealing with
const usesSidebarLayout = content.includes('SidebarLayout');

if (usesSidebarLayout) {
    // === New structure: SidebarLayout props-based ===

    // 1. Add FiTerminal import
    if (content.includes('FiSettings')) {
        content = content.replace(
            /import \{ FiSettings \} from 'react-icons\/fi';/,
            "import { FiSettings, FiTerminal } from 'react-icons/fi';"
        );
    } else {
        // Add import at the top
        content = content.replace(
            /(import React[^\n]+\n)/,
            "$1import { FiTerminal } from 'react-icons/fi';\n"
        );
    }

    // 2. Add isTauri import
    if (!content.includes('isTauri')) {
        content = content.replace(
            /(import \{ useTranslation[^\n]+)/,
            "$1\nimport { isTauri } from '@/app/_common/api/core/platform';"
        );
    }

    // 3. Add state + handler after quickLogout
    // NOTE: useAuth() already provides user.access_token — no need for document.cookie
    content = content.replace(
        /const \{ quickLogout \} = useQuickLogout\(\);/,
        `const { quickLogout } = useQuickLogout();

    const [isTauriApp, setIsTauriApp] = useState(false);
    useEffect(() => {
        setIsTauriApp(isTauri());
        // Listen for navigate events from AI CLI
        if (isTauri()) {
            import('@tauri-apps/api/event').then(({ listen }) => {
                listen('navigate', (event: any) => {
                    const path = event.payload?.path;
                    if (path) {
                        console.log('[AI CLI] Navigate to:', path);
                        router.push(path);
                    }
                });
            }).catch(() => {});
        }
    }, []);

    const openCliWindow = async () => {
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            // Get token directly from CookieProvider context (user object)
            const token = user?.access_token || undefined;
            console.log('[AI CLI] Opening with token:', token ? token.substring(0, 20) + '...' : 'NONE');
            await invoke('open_cli_window', { xgenToken: token });
        } catch (e) {
            console.error('Failed to open CLI window:', e);
        }
    };`
    );

    // 4. Patch afterSections to include AI CLI button
    // Find the afterSections definition and wrap it to also include the CLI button
    content = content.replace(
        /const afterSections = ([\s\S]*?)(:\s*null;)/,
        (match, middle, end) => {
            return `const trainAfterSection = ${middle}${end}

    const afterSections = (
        <>
            {trainAfterSection}
            {isTauriApp && (
                <button
                    type="button"
                    className={\`\${styles.sidebarToggle}\`}
                    onClick={openCliWindow}
                    data-sidebar-trigger
                >
                    <span className={styles.toggleSectionIcon}>
                        <FiTerminal />
                    </span>
                    <span className={styles.toggleTitle}>AI CLI</span>
                </button>
            )}
        </>
    );`;
        }
    );

} else {
    // === Legacy structure: direct JSX render ===

    // 1. Add FiTerminal import
    content = content.replace('FiLogOut }', 'FiLogOut, FiTerminal }');

    // 2. Add isTauri import
    content = content.replace(
        /import { useTranslation[^\n]+/,
        (match) => `${match}\nimport { isTauri } from '@/app/_common/api/core/platform';`
    );

    // 3. Add useEffect if missing
    if (!content.includes('useEffect')) {
        content = content.replace(
            /import React, \{ useState, useMemo \}/,
            `import React, { useState, useMemo, useEffect }`
        );
    }

    // 4. Add isTauriApp state + openCliWindow handler
    content = content.replace(
        /const \{ quickLogout \} = useQuickLogout\(\);/,
        `const { quickLogout } = useQuickLogout();
    const [isTauriApp, setIsTauriApp] = React.useState(false);
    React.useEffect(() => { setIsTauriApp(isTauri()); }, []);

    const openCliWindow = async () => {
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            const getCookie = (name) => {
                const match = document.cookie.match(new RegExp('(^| )' + name + '=([^;]+)'));
                return match ? match[2] : null;
            };
            const token = getCookie('access_token') || undefined;
            await invoke('open_cli_window', { xgenToken: token });
        } catch (e) {
            console.error('Failed to open CLI window:', e);
        }
    };`
    );

    // 5. Add AI CLI button
    content = content.replace(
        /(isPopoverOpen=\{openPopover === 'mlModel'\}\s*\/>[\s\S]*?\)\})\s*(\n(\s*)<\/div>)/,
        (match, p1, p2, p3) => {
            return `${p1}
${p3}{isTauriApp && (
${p3}    <button
${p3}        type="button"
${p3}        className={styles.sidebarToggle}
${p3}        onClick={openCliWindow}
${p3}        data-sidebar-trigger
${p3}    >
${p3}        <span className={styles.toggleSectionIcon}>
${p3}            <FiTerminal />
${p3}        </span>
${p3}        <span className={styles.toggleTitle}>AI CLI</span>
${p3}    </button>
${p3})}
${p2}`;
        }
    );
}

fs.writeFileSync(sidebarPath, content);
console.log('[OK] XgenSidebar 패치 완료');
