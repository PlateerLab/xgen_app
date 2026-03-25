#!/usr/bin/env node
/**
 * XgenSidebar.tsx에 AI CLI 버튼을 패치하는 스크립트
 * 사용법: node patch-sidebar-cli.js <frontend-dir>
 */
const fs = require('fs');
const path = require('path');

const frontendDir = process.argv[2];
if (!frontendDir) {
    console.error('Usage: node patch-sidebar-cli.js <frontend-dir>');
    process.exit(1);
}

const sidebarPath = path.join(frontendDir, 'src/app/main/sidebar/XgenSidebar.tsx');
if (!fs.existsSync(sidebarPath)) {
    console.log('[WARN] XgenSidebar.tsx not found');
    process.exit(0);
}

let content = fs.readFileSync(sidebarPath, 'utf8');
if (content.includes('open_cli_window')) {
    console.log('[INFO] XgenSidebar 이미 패치됨');
    process.exit(0);
}

console.log('[PATCH] XgenSidebar에 AI CLI 버튼 추가...');

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
            await invoke('open_cli_window');
        } catch (e) {
            console.error('Failed to open CLI window:', e);
        }
    };`
);

// 5. Add AI CLI button after mlModel section (before closing </div> of sidebarSectionList)
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

fs.writeFileSync(sidebarPath, content);
console.log('[OK] XgenSidebar 패치 완료');
