# xgen_app Dockerfile
# xgen-frontend 저장소에서 최신 소스를 가져와 빌드

# ===== Stage 1: 프론트엔드 소스 가져오기 및 빌드 =====
FROM node:20-alpine AS builder

# Git 설치 (소스 클론용)
RUN apk add --no-cache git

WORKDIR /app

# 빌드 인자
ARG FRONTEND_REPO=http://gitlab.x2bee.com/tech-team/ai-team/xgen/xgen-frontend.git
ARG FRONTEND_BRANCH=main

# xgen-frontend 소스 클론
RUN git clone --depth 1 --branch ${FRONTEND_BRANCH} ${FRONTEND_REPO} frontend

WORKDIR /app/frontend

# next.config.ts에 output: 'standalone' 설정 추가 (Docker 배포용)
RUN sed -i "s/const nextConfig: NextConfig = {/const nextConfig: NextConfig = {\n    output: 'standalone',/" next.config.ts

# 의존성 설치
RUN npm ci --legacy-peer-deps

# 환경 변수 파일이 있으면 복사 (빌드 시 필요한 경우)
# COPY .env.production .env.local

# Next.js 빌드
ENV NODE_OPTIONS="--max-old-space-size=4096"
RUN npm run build

# ===== Stage 2: 프로덕션 실행 환경 =====
FROM node:20-alpine AS runner

WORKDIR /app

ENV NODE_ENV=production

# 보안: non-root 사용자 생성
RUN addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 nextjs

# 필수 파일만 복사
COPY --from=builder /app/frontend/public ./public
COPY --from=builder --chown=nextjs:nodejs /app/frontend/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/frontend/.next/static ./.next/static

# 빌드 정보 기록
ARG BUILD_DATE
ARG GIT_COMMIT
RUN echo "BUILD_DATE=${BUILD_DATE}" > /app/build-info.txt && \
    echo "GIT_COMMIT=${GIT_COMMIT}" >> /app/build-info.txt

USER nextjs

EXPOSE 3000

ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["node", "server.js"]
