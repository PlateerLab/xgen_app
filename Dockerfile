# xgen_app Dockerfile
# Optimized multi-stage build for Next.js standalone deployment

# ===== Stage 1: Dependencies & Build =====
FROM node:20-alpine AS builder

# Install dependencies for native modules (sharp, canvas, etc.)
RUN apk add --no-cache \
    git \
    libc6-compat \
    python3 \
    make \
    g++

WORKDIR /app

# Build arguments
ARG FRONTEND_REPO=http://gitlab.x2bee.com/tech-team/ai-team/xgen/xgen-frontend.git
ARG FRONTEND_BRANCH=main
ARG USE_LOCAL_SOURCE=false

# Option 1: Clone from git repository
RUN if [ "$USE_LOCAL_SOURCE" = "false" ]; then \
    git clone --depth 1 --branch ${FRONTEND_BRANCH} ${FRONTEND_REPO} frontend; \
    fi

# Option 2: Use local frontend source (when USE_LOCAL_SOURCE=true)
COPY frontend/ /app/frontend-local/
RUN if [ "$USE_LOCAL_SOURCE" = "true" ]; then \
    mv /app/frontend-local /app/frontend; \
    else \
    rm -rf /app/frontend-local; \
    fi

WORKDIR /app/frontend

# Add standalone output configuration for Docker deployment
RUN if ! grep -q "output.*standalone" next.config.ts 2>/dev/null; then \
    sed -i "s/const nextConfig: NextConfig = {/const nextConfig: NextConfig = {\n    output: 'standalone',/" next.config.ts || \
    sed -i "s/const nextConfig = {/const nextConfig = {\n    output: 'standalone',/" next.config.ts || \
    sed -i "s/export default {/export default {\n    output: 'standalone',/" next.config.ts; \
    fi

# Install dependencies with cache optimization
RUN npm ci --legacy-peer-deps --prefer-offline

# Build environment
ENV NODE_OPTIONS="--max-old-space-size=4096"
ENV NEXT_TELEMETRY_DISABLED=1

# Build Next.js application
RUN npm run build

# ===== Stage 2: Production Runtime =====
FROM node:20-alpine AS runner

WORKDIR /app

# Install runtime dependencies only
RUN apk add --no-cache \
    libc6-compat \
    wget \
    && rm -rf /var/cache/apk/*

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

# Security: Create non-root user
RUN addgroup --system --gid 1001 nodejs \
    && adduser --system --uid 1001 nextjs

# Copy only necessary production files
COPY --from=builder /app/frontend/public ./public
COPY --from=builder --chown=nextjs:nodejs /app/frontend/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/frontend/.next/static ./.next/static

# Build metadata
ARG BUILD_DATE
ARG GIT_COMMIT
LABEL org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.revision="${GIT_COMMIT}" \
      org.opencontainers.image.title="xgen-frontend" \
      org.opencontainers.image.vendor="Plateer"

RUN echo "BUILD_DATE=${BUILD_DATE:-$(date -Iseconds)}" > /app/build-info.txt \
    && echo "GIT_COMMIT=${GIT_COMMIT:-unknown}" >> /app/build-info.txt

# Switch to non-root user
USER nextjs

EXPOSE 3000

ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3000/ || exit 1

CMD ["node", "server.js"]
