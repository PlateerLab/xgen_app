# Tauri 앱 빌드 주의사항

이 문서는 XGEN Desktop App을 Tauri로 빌드할 때 발생할 수 있는 문제점과 해결 방법을 정리한 것입니다.

## 1. 쿠키 vs localStorage

### 문제
- **Dev 모드**: `http://localhost:3000`에서 실행되어 `document.cookie`가 정상 작동
- **Release 빌드**: `tauri://localhost` 프로토콜에서 실행되어 **`document.cookie`가 작동하지 않음**

### 해결
`cookieUtils.js`에서 Tauri 환경을 감지하여 **localStorage**를 사용하도록 수정:

```javascript
const isTauriEnv = () => {
    if (typeof window === 'undefined') return false;
    return '__TAURI__' in window || '__TAURI_INTERNALS__' in window;
};

export const setCookieAuth = (key, value) => {
    if (isTauriEnv()) {
        localStorage.setItem(key, value);
        return;
    }
    // 웹에서는 기존 쿠키 사용
    setCookie(key, value, days, options);
};

export const getAuthCookie = (key) => {
    if (isTauriEnv()) {
        return localStorage.getItem(key);
    }
    return getCookie(key);
};
```

### 영향 받는 파일
- `frontend/src/app/_common/utils/cookieUtils.js`

---

## 2. Tauri HTTP 플러그인 버전 일치

### 문제
프론트엔드 `@tauri-apps/plugin-http`와 Rust `tauri-plugin-http` 버전이 불일치하면 다음 에러 발생:
```
TypeError: invalid args `streamChannel` for command `fetch_read_body`
```

### 해결
버전을 정확히 일치시킴:

**frontend/package.json**:
```json
"@tauri-apps/plugin-http": "^2.5.5"
```

**src-tauri/Cargo.toml**:
```toml
tauri-plugin-http = "2.5.5"
```

### 확인 방법
```bash
# 프론트엔드 버전 확인
grep "plugin-http" frontend/package-lock.json | head -3

# Rust 버전 확인
grep -A2 'name = "tauri-plugin-http"' src-tauri/Cargo.lock
```

---

## 3. Capabilities 권한 설정

### 문제
HTTP 플러그인 사용 시 다음 에러 발생:
```
Command plugin:http|fetch_cancel_body not allowed by ACL
```

### 해결
`src-tauri/capabilities/default.json`에 필요한 권한 추가:

```json
{
  "permissions": [
    "http:default",
    {
      "identifier": "http:allow-fetch",
      "allow": [
        { "url": "http://**" },
        { "url": "https://**" }
      ]
    },
    {
      "identifier": "http:allow-fetch-cancel",
      "allow": [
        { "url": "http://**" },
        { "url": "https://**" }
      ]
    },
    {
      "identifier": "http:allow-fetch-read-body",
      "allow": [
        { "url": "http://**" },
        { "url": "https://**" }
      ]
    },
    {
      "identifier": "http:allow-fetch-send",
      "allow": [
        { "url": "http://**" },
        { "url": "https://**" }
      ]
    },
    "http:allow-fetch-cancel-body",
    "http:allow-fetch-read-body"
  ]
}
```

---

## 4. Release 빌드 디버깅

### DevTools 활성화

Release 빌드에서 WebView DevTools를 열려면:

**src-tauri/Cargo.toml**:
```toml
tauri = { version = "2.8.5", features = ["devtools"] }
```

**src-tauri/src/lib.rs** (setup 훅 내부):
```rust
// Enable devtools in release build for debugging
if let Some(window) = app.get_webview_window("main") {
    window.open_devtools();
    log::info!("DevTools opened for debugging");
}
```

> **주의**: 프로덕션 배포 시에는 이 코드를 제거하거나 조건부로 비활성화하세요.

---

## 5. 빌드 명령어

### 개발 모드
```bash
npm run tauri dev
```

### Release 빌드 (번들 없이)
```bash
cargo tauri build --no-bundle
```

### Release 빌드 (전체)
```bash
cargo tauri build
```

### 빌드된 앱 실행 (macOS)
```bash
./src-tauri/target/release/app
# 또는 번들된 앱
./src-tauri/target/release/bundle/macos/XGEN.app/Contents/MacOS/app
```

---

## 6. 일반적인 디버깅 체크리스트

Release 빌드에서 API 호출이 실패할 때:

1. **DevTools 콘솔 확인**
   - Safari > 개발자용 > [Mac 이름] > XGEN (dev 모드 필요)
   - 또는 `devtools` feature 활성화 후 Release 빌드

2. **확인할 로그**
   ```
   [getTauriFetch] HTTP plugin imported successfully  ← 플러그인 로드 성공
   [tauriApiClient] Token from cookie: "eyJ..."      ← 토큰 읽기 성공
   [tauriApiClient] Response status: 200             ← API 호출 성공
   ```

3. **흔한 문제**
   | 증상 | 원인 | 해결 |
   |------|------|------|
   | `Token from cookie: null` | 쿠키 미작동 | localStorage 사용 |
   | `fetch_read_body` 에러 | 플러그인 버전 불일치 | 버전 맞춤 |
   | `not allowed by ACL` | 권한 부족 | capabilities 추가 |
   | `localhost:3000` 연결 실패 | 잘못된 빌드 | `cargo tauri build` 사용 |

---

## 7. 환경 변수

빌드 시 환경 변수가 정적으로 번들됩니다:

```bash
# frontend/.env.local 또는 빌드 시 설정
NEXT_PUBLIC_BACKEND_HOST=https://xgen-backend-gateway.x2bee.io
```

Connected 모드에서는 앱 설정의 `serverUrl`이 우선 사용됩니다.

---

## 변경 이력

| 날짜 | 내용 |
|------|------|
| 2026-01-10 | 최초 작성 - 쿠키/localStorage, HTTP 플러그인 버전, capabilities 문제 해결 |
