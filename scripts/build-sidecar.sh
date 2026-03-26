#!/bin/bash

# graph-tool-call PyInstaller sidecar 빌드 스크립트
# Tauri externalBin 규칙: binaries/{name}-{target_triple}

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BINARIES_DIR="$PROJECT_ROOT/src-tauri/binaries"
GRAPH_TOOL_CALL_DIR="${GRAPH_TOOL_CALL_DIR:-$HOME/projects/app/graph-tool-call}"

# 색상 정의
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

print_step() { echo -e "\n${GREEN}==== $1 ====${NC}\n"; }
print_warn() { echo -e "${YELLOW}[WARN] $1${NC}"; }
print_error() { echo -e "${RED}[ERROR] $1${NC}"; }

# Tauri target triple 감지
get_target_triple() {
    local arch=$(uname -m)
    local os=$(uname -s)

    case "$os" in
        Linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                *)       print_error "Unsupported arch: $arch"; exit 1 ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64)  echo "x86_64-apple-darwin" ;;
                arm64)   echo "aarch64-apple-darwin" ;;
                *)       print_error "Unsupported arch: $arch"; exit 1 ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "x86_64-pc-windows-msvc"
            ;;
        *)
            print_error "Unsupported OS: $os"
            exit 1
            ;;
    esac
}

TARGET_TRIPLE=$(get_target_triple)
BINARY_NAME="graph-tool-call-${TARGET_TRIPLE}"

print_step "graph-tool-call sidecar 빌드 (target: ${TARGET_TRIPLE})"

# 1. graph-tool-call 소스 확인
if [ ! -d "$GRAPH_TOOL_CALL_DIR" ]; then
    print_error "graph-tool-call 소스가 없습니다: $GRAPH_TOOL_CALL_DIR"
    print_warn "GRAPH_TOOL_CALL_DIR 환경변수로 경로를 지정하세요."
    exit 1
fi

# 2. PyInstaller 확인
if ! command -v pyinstaller &> /dev/null; then
    print_warn "PyInstaller가 없습니다. 설치 중..."
    pip install pyinstaller
fi

# 3. PyInstaller 빌드
print_step "PyInstaller 빌드"
cd "$GRAPH_TOOL_CALL_DIR"

pyinstaller \
    --onefile \
    --name graph-tool-call \
    --collect-submodules graph_tool_call \
    --strip \
    --noconfirm \
    --distpath "$GRAPH_TOOL_CALL_DIR/dist" \
    graph_tool_call/__main__.py

# 4. 빌드 결과 검증
BUILT_BINARY="$GRAPH_TOOL_CALL_DIR/dist/graph-tool-call"
if [ ! -f "$BUILT_BINARY" ]; then
    print_error "빌드 실패: $BUILT_BINARY 가 없습니다."
    exit 1
fi

# 버전 확인
VERSION=$("$BUILT_BINARY" --version 2>&1 || true)
echo "빌드된 바이너리: $VERSION"

# 5. Tauri binaries 디렉토리로 복사
print_step "Tauri sidecar 배치"
mkdir -p "$BINARIES_DIR"
cp "$BUILT_BINARY" "$BINARIES_DIR/$BINARY_NAME"
chmod +x "$BINARIES_DIR/$BINARY_NAME"

BINARY_SIZE=$(du -h "$BINARIES_DIR/$BINARY_NAME" | cut -f1)

echo ""
echo -e "${GREEN}✅ 빌드 완료!${NC}"
echo "  바이너리: $BINARIES_DIR/$BINARY_NAME"
echo "  크기: $BINARY_SIZE"
echo "  버전: $VERSION"
echo ""
echo "Tauri 앱 빌드 시 자동으로 번들됩니다."
