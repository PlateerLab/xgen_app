import type { NextConfig } from 'next';

const nextConfig: NextConfig = {
    // Tauri를 위한 정적 익스포트 설정
    output: 'export',
    trailingSlash: true,
    // 정적 빌드에서 로컬 파일로 로드할 수 있도록 자산 경로를 프로덕션에서만 상대 경로로 설정
    // 개발 모드에서는 빈 문자열로 두어 Dev 서버가 정상 동작하도록 합니다.
    assetPrefix: process.env.NODE_ENV === 'production' ? './' : '',

    // 이미지 최적화 비활성화 (Tauri에서 필요)
    images: {
        unoptimized: true,
    },
    // NOTE: Next.js `rewrites`는 정적 `next export` 결과물(프로덕션)에서는 동작하지 않습니다.
    // Tauri 환경에서는 rewrites에 의존하지 않고 프런트엔드에서 명시적인
    // `API_BASE_URL` (환경변수)로 백엔드에 접근해야 합니다. 따라서 rewrites를 제거했습니다.
};

export default nextConfig;
