# XGEN App - Tauri + Next.js 데스크탑 앱 설정 및 실행 가이드

## 개요
XGEN은 Next.js로 개발된 AI 워크플로우 플랫폼을 Tauri로 래핑하여 데스크탑 애플리케이션으로 만든 프로젝트입니다. 드래그 앤 드롭으로 AI 파이프라인을 구축하고 실시간으로 상호작용할 수 있는 차세대 AI 워크플로우 플랫폼입니다.

## 사전 요구사항

### 1. Rust 설치
```bash
# Rust 설치 (공식 인스톨러 사용)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 설치 후 환경 변수 적용
source ~/.cargo/env

# 설치 확인
rustc --version
cargo --version
```

### 2. Node.js 설치 (18.17 이상)
#### macOS (Homebrew)
```bash
brew install node
```

#### Ubuntu/Debian
```bash
sudo apt update
sudo apt install nodejs npm
```

#### Windows (Chocolatey)
```bash
choco install nodejs
```

#### 공식 인스톨러
[Node.js 공식 웹사이트](https://nodejs.org/)에서 LTS 버전 다운로드 및 설치

### 3. 시스템별 Tauri 의존성

#### macOS
```bash
# Xcode Command Line Tools 설치
xcode-select --install
```

#### Ubuntu/Debian
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
```

#### Arch Linux
```bash
sudo pacman -S webkit2gtk base-devel curl wget openssl gtk3 libappindicator-gtk3 librsvg
```

#### Fedora
```bash
sudo dnf groupinstall "C Development Tools and Libraries"
sudo dnf install webkit2gtk3-devel openssl-devel curl wget gtk3-devel libappindicator-gtk3-devel librsvg2-devel
```

#### Windows
- Microsoft Visual Studio C++ Build Tools 설치
- 또는 Visual Studio Community 2019/2022 (C++ 워크로드 포함)
- WebView2 런타임이 설치되어 있는지 확인 (Windows 11에는 기본 설치됨)

## 프로젝트 설정

### 1. 저장소 클론
```bash
# X2bee의 xgen_app 프로젝트 클론
git clone https://github.com/X2bee/xgen_app.git
cd xgen_app
```

### 2. Tauri CLI 설치
```bash
# Rust를 통한 Tauri CLI 설치 (권장)
cargo install tauri-cli

# 또는 npm을 통해 설치
npm install -g @tauri-apps/cli
```

### 3. 프론트엔드 의존성 설치
```bash
# npm 사용
npm install

# 또는 yarn 사용
yarn install

# 또는 pnpm 사용
pnpm install
```

### 4. Rust 의존성 설치 (자동)
Tauri CLI가 필요시 자동으로 Rust 의존성을 설치합니다.

## 개발 환경 실행

### 1. Tauri 개발 모드 실행
```bash
# Tauri 개발 서버 시작 (프론트엔드 + 백엔드 통합)
npm run tauri dev

# 또는 yarn 사용
yarn tauri dev

# 또는 Cargo 직접 사용
cargo tauri dev
```

이 명령어는:
- Next.js 개발 서버를 시작합니다 (보통 http://localhost:3000)
- Rust 백엔드를 컴파일합니다
- Tauri 데스크탑 앱 윈도우를 엽니다

### 2. 개발 중 핫 리로드
- **프론트엔드 변경**: Next.js 파일 수정 시 자동 리로드
- **백엔드 변경**: `src-tauri/src/` 내 Rust 파일 수정 시 자동 재컴파일 및 앱 재시작

## 빌드 및 배포

### 1. 프로덕션 빌드
```bash
# 전체 애플리케이션 빌드 (프론트엔드 + 데스크탑 앱)
npm run tauri build

# 또는 yarn 사용
yarn tauri build

# 또는 Cargo 직접 사용
cargo tauri build
```

### 2. 빌드 결과물 위치
빌드 완료 후 다음 위치에서 설치 파일을 찾을 수 있습니다:
- **Windows**: `src-tauri/target/release/bundle/msi/` 또는 `src-tauri/target/release/bundle/nsis/`
- **macOS**: `src-tauri/target/release/bundle/dmg/` 및 `src-tauri/target/release/bundle/macos/`
- **Linux**: `src-tauri/target/release/bundle/deb/`, `src-tauri/target/release/bundle/rpm/`, `src-tauri/target/release/bundle/appimage/`
