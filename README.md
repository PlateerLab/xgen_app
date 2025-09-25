# PLATEERAG - AI 워크플로우 시각적 에디터 설정 및 실행 가이드
<img width="1705" height="1101" alt="image" src="https://github.com/user-attachments/assets/01ac0364-d037-4032-a673-577cf02d40bb" />


## 개요
PLATEERAG는 드래그 앤 드롭 노드로 AI 기반 애플리케이션을 구축할 수 있는 시각적 워크플로우 에디터입니다. Next.js 기반으로 개발되었으며, Tauri를 통해 데스크탑 애플리케이션으로도 사용할 수 있습니다.

## 주요 기능
- **시각적 워크플로우 에디터**: 드래그 앤 드롭으로 AI 워크플로우 구성
- **AI 노드 시스템**: 다양한 AI 모델과 도구들을 노드로 연결
- **캔버스 기반 인터페이스**: 직관적인 워크플로우 설계
- **자동화 도구**: AI 기반 자동화 워크플로우 생성

## 사전 요구사항

### 1. Node.js 설치 (18.17 이상)
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

### 2. Rust 설치 (Tauri 데스크탑 앱 사용 시)
```bash
# Rust 설치
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 설치 확인
rustc --version
cargo --version
```

### 3. 시스템별 Tauri 의존성 (데스크탑 앱 사용 시)

#### macOS
```bash
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

#### Windows
- Visual Studio C++ Build Tools 설치
- WebView2 런타임 설치

## 프로젝트 설정

### 1. 저장소 클론
```bash
git clone https://github.com/X2bee/xgen_app.git
cd xgen_app
```

### 2. 의존성 설치
```bash
# npm 사용
npm install

# 또는 yarn 사용
yarn install

# 또는 pnpm 사용
pnpm install
```

설치 과정에서 다음과 같은 메시지가 나타날 수 있습니다:
- Husky 훅 설정: `> plateerag@0.1.0 prepare > husky`
- 보안 취약점 알림: `npm audit` 명령어로 확인 가능

### 3. 환경 변수 설정 (필수)
```bash
# sample.env 파일을 .env로 복사
cp sample.env .env
```

**⚠️ 중요**: 백엔드와 통신하기 위해서는 반드시 `sample.env` 파일을 `.env`로 변경한 후 배포해야 합니다.

## 환경 설정 (필수)

**⚠️ 중요**: 애플리케이션을 실행하기 전에 반드시 환경 변수를 설정해야 합니다.

```bash
# sample.env 파일을 .env로 복사
cp sample.env .env

# 또는 수동으로 파일명 변경
mv sample.env .env
```

`.env` 파일을 열어서 필요한 환경 변수들을 설정하세요:
- API 엔드포인트 URL
- 데이터베이스 연결 정보
- 외부 서비스 API 키 등

백엔드와의 통신을 위해 이 단계는 **필수**입니다.

## 실행 방법

### 1. 웹 개발 서버 실행 (권장)
```bash
# Next.js 개발 서버 시작
npm run dev
```

브라우저에서 [http://localhost:3000](http://localhost:3000)에 접속

### 2. Tauri 데스크탑 앱 실행
```bash
# npx를 사용하여 Tauri CLI 실행
npx tauri dev

# 또는 전역 Tauri CLI 설치 후 실행
cargo install tauri-cli
cargo tauri dev
```

**참고**: 현재 `package.json`에 `tauri` 스크립트가 정의되어 있지 않으므로 `npx tauri dev` 또는 `cargo tauri dev` 명령어를 직접 사용해야 합니다.

## 빌드

### 1. 웹 애플리케이션 빌드
```bash
# Next.js 프로덕션 빌드
npm run build

# 빌드 후 프로덕션 서버 시작
npm run start

# 정적 파일 내보내기
npm run export
```

### 2. Tauri 데스크탑 앱 빌드
```bash
# 데스크탑 앱 빌드
npx tauri build

# 또는 cargo 직접 사용
cargo tauri build
```

빌드 결과물:
- **Windows**: `src-tauri/target/release/bundle/msi/` 또는 `nsis/`
- **macOS**: `src-tauri/target/release/bundle/dmg/` 및 `macos/`
- **Linux**: `src-tauri/target/release/bundle/deb/`, `rpm/`, `appimage/`

## 개발 도구

### 코드 품질 관리
```bash
# ESLint 검사
npm run lint

# ESLint 자동 수정
npm run lint:fix

# Prettier 코드 포맷팅
npm run format

# 챗봇 임베드 빌드
npm run build:embed
```

### Git 훅 설정
프로젝트에는 Husky가 설정되어 있어 커밋 시 자동으로 코드 품질 검사가 실행됩니다:
- 코드 린팅 (ESLint)
- 코드 포맷팅 (Prettier)
- 타입 체크 (TypeScript)

## 프로젝트 구조
```
plateerag/
├── src/                           # Next.js 소스 코드
│   ├── app/                       # Next.js App Router
│   │   ├── page.tsx              # 메인 페이지
│   │   ├── layout.tsx            # 루트 레이아웃
│   │   └── globals.css           # 글로벌 스타일
│   ├── components/               # React 컴포넌트
│   ├── lib/                      # 유틸리티 및 라이브러리
│   └── styles/                   # 스타일 파일
├── src-tauri/                    # Tauri 백엔드 (Rust)
│   ├── src/
│   │   ├── main.rs              # Tauri 메인 진입점
│   │   └── lib.rs               # Tauri 라이브러리
│   ├── Cargo.toml               # Rust 의존성
│   ├── tauri.conf.json          # Tauri 설정
│   └── icons/                   # 앱 아이콘
├── public/                       # 정적 파일
├── package.json                  # Node.js 의존성 및 스크립트
├── next.config.js                # Next.js 설정
├── tailwind.config.js            # Tailwind CSS 설정
├── tsconfig.json                 # TypeScript 설정
├── .eslintrc.js                  # ESLint 설정
├── .prettierrc                   # Prettier 설정
└── README.md
```

## 기술 스택

### 프론트엔드
- **Next.js 15.3.2**: React 기반 풀스택 프레임워크
- **React 19**: 최신 React 기능
- **TypeScript**: 타입 안전성
- **Tailwind CSS**: 유틸리티 기반 CSS 프레임워크
- **Framer Motion**: 애니메이션 라이브러리
- **React Hot Toast**: 알림 시스템

### 시각화 및 차트
- **D3.js 7.9.0**: 데이터 시각화
- **Chart.js 4.5.0**: 차트 라이브러리
- **React ChartJS 2**: React Chart.js 래퍼

### 문서 처리
- **PDF.js**: PDF 뷰어 (`pdfjs-dist`)
- **React PDF**: React PDF 컴포넌트
- **Mammoth**: Word 문서 변환
- **KaTeX**: 수학 수식 렌더링

### 개발 도구
- **ESLint**: 코드 품질 관리
- **Prettier**: 코드 포맷팅
- **Husky**: Git 훅 관리
- **lint-staged**: 스테이지된 파일 린팅
- **esbuild**: 빠른 번들링

### 데스크탑 (선택사항)
- **Tauri 2.8.4**: Rust 기반 데스크탑 앱 프레임워크

## Tauri 스크립트 추가 (선택사항)

`package.json`의 `scripts` 섹션에 Tauri 관련 스크립트를 추가하려면:

```json
{
  "scripts": {
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build",
    "tauri:info": "tauri info"
  }
}
```

추가 후에는 다음과 같이 실행할 수 있습니다:
```bash
npm run tauri:dev
npm run tauri:build
```

## 트러블슈팅

### 1. 의존성 설치 오류
```bash
# npm 캐시 정리
npm cache clean --force
rm -rf node_modules package-lock.json
npm install
```

### 2. Tauri 관련 오류
```bash
# Rust 업데이트
rustup update stable

# Tauri CLI 재설치
cargo install tauri-cli --force
```

### 3. 보안 취약점 해결
```bash
# 자동 수정 (주의: 호환성 문제 발생 가능)
npm audit fix

# 강제 수정 (주의: 중대한 변경 사항 포함)
npm audit fix --force
```

### 4. 포트 충돌
```bash
# 다른 포트로 실행
PORT=3001 npm run dev
```

### 5. TypeScript 오류
```bash
# .next 캐시 삭제
rm -rf .next
npm run dev
```

## 환경 변수 설정

### 기본 환경 변수 파일
프로젝트에는 `sample.env` 파일이 포함되어 있습니다. 이를 복사하여 사용하세요:

```bash
# sample.env를 .env로 복사
cp sample.env .env
```

### 추가 환경 변수
필요에 따라 `.env.local` 파일을 생성하여 추가 환경 변수를 설정할 수 있습니다:

```bash
# .env.local
NEXT_PUBLIC_API_URL=http://localhost:8000
NEXT_PUBLIC_APP_ENV=development
```

## 배포

### Vercel 배포 (웹 앱)
```bash
# Vercel CLI 설치
npm install -g vercel

# 배포
vercel
```

### 데스크탑 앱 배포
```bash
# 각 플랫폼별 빌드
npm run tauri:build

# GitHub Releases에 업로드하거나
# 각 플랫폼의 앱 스토어에 배포
```

## 기여하기

1. 이 저장소를 포크합니다
2. 기능 브랜치를 생성합니다 (`git checkout -b feature/amazing-feature`)
3. 변경사항을 커밋합니다 (`git commit -m 'Add amazing feature'`)
4. 브랜치에 푸시합니다 (`git push origin feature/amazing-feature`)
5. Pull Request를 생성합니다

## 라이선스

이 프로젝트는 개인 라이선스 하에 배포됩니다.

## 개발팀

- **Plateer AI-LAB**
- **CocoRoF**
- **haesookimDev**

## 추가 리소스

- [Next.js 공식 문서](https://nextjs.org/docs)
- [Tauri 공식 문서](https://tauri.app/)
- [React 공식 문서](https://react.dev/)
- [TypeScript 공식 문서](https://www.typescriptlang.org/docs/)

---

Made with ❤️ by Plateer AI-LAB
