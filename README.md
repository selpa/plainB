# Plain Browser

**기본에 충실한 브라우저.**

무분별한 외부 광고 요청을 차단하고, 웹 콘텐츠를 편하게 보는 것을 목표로 하는 Rust 기반 Windows 브라우저입니다. 복잡한 기능보다 기본 탐색과 광고 차단에 집중합니다.

현재 버전은 **0.1.0 개발·시험 배포본**입니다. Windows x64용으로 빌드하며, 웹페이지 렌더링에는 Microsoft Edge WebView2를 사용합니다. 자체 렌더링 엔진을 구현한 프로젝트는 아닙니다.

## 주요 기능

- 주소 입력, 뒤로·앞으로 이동, 새로고침
- 탭 추가·전환·닫기 및 마우스 드래그로 순서 변경
- 탭에 사이트 파비콘 표시, 로딩 중 회전 표시, 아이콘이 없으면 기본 지구본 표시
- 다른 호스트의 링크를 새 탭으로 열어 원래 페이지 보존
- 마우스를 올린 링크의 주소를 하단 왼쪽에 표시
- `adblock-rust`와 EasyList를 이용한 외부 광고 요청 차단
- 하단 상태 표시줄의 광고차단 ON / OFF 전환
- 주소창 옆 팝업 관리: 한 번 허용, 사이트별 항상 허용, 허용 취소
- PB 로고, 38×38 정사각형 탐색 버튼과 선 두께 1.8의 SVG 아이콘을 사용하는 간결한 화면

시작 화면은 `+` 버튼으로 새 탭에서 엽니다. 주소창 옆 제품명과 별도 홈 버튼은 표시하지 않습니다.

## 광고 차단 방식

페이지에서 발생하는 외부 요청을 EasyList 네트워크 규칙과 대조하여 차단합니다. 광고 차단은 기본으로 켜져 있습니다.

- 최상위 페이지 탐색과 같은 사이트의 콘텐츠 요청은 유지합니다.
- 게시자가 HTML에 직접 넣은 콘텐츠를 분석하거나 숨기지 않습니다.
- CSS로 요소 숨기기, 스크립틀릿, 본문 추출은 적용하지 않습니다.
- 모든 광고가 제거되는 것을 보장하지 않으며, 공개 사이트 전체의 차단율은 측정하지 않았습니다.

원본 목록은 [`filters/easylist.txt`](filters/easylist.txt)에 있으며 빌드할 때 실행 파일에 포함됩니다. 현재 실행 중 외부 TXT 파일을 다시 읽거나 목록을 자동으로 갱신하는 기능은 없습니다. 목록을 변경한 경우 다시 빌드해야 합니다.

## 링크와 탭 동작

| 동작 | 처리 |
| --- | --- |
| 현재 페이지와 같은 호스트의 링크 클릭 | 현재 탭에서 이동 |
| 다른 호스트의 링크 클릭 | 새 탭에서 열고 전환 |
| 서브도메인이 다른 링크 클릭 | 새 탭에서 열고 전환 |
| 주소창에 주소 직접 입력 | 현재 탭에서 이동 |
| 뒤로·앞으로, 새로고침, 탐색 중 리디렉션 | 현재 탭 유지 |
| 자동 팝업 | 기본 차단 후 팝업 아이콘에서 허용 여부 선택 |

예를 들어 `news.example.com`에서 `www.example.com`으로 이동하는 링크는 새 탭으로 엽니다. 같은 호스트에서 경로·포트·HTTP/HTTPS가 달라지는 것은 새 탭 기준에 포함하지 않습니다.

링크를 새 탭으로 여는 호스트 비교와 광고 차단의 같은 사이트 판정은 서로 별개의 정책입니다.

### 팝업 관리

차단된 팝업이 있으면 주소창 옆 팝업 아이콘에 표시합니다. 아이콘을 누르면 대상 주소를 확인하고 **한 번 허용 / 이 사이트 항상 허용 / 닫기**를 선택할 수 있습니다. 허용한 팝업은 새 탭에서 엽니다. 항상 허용한 사이트에서는 같은 메뉴에 **허용 취소**가 표시됩니다.

권한은 서브도메인을 포함한 전체 호스트명을 기준으로 실행 파일 옆 `data/webview2/popup-permissions.json`에 저장합니다. 반복 요청은 첫 번째 보류 주소 하나로 묶으며, 페이지 이동 시 보류 요청을 지웁니다. 항상 허용한 페이지도 무한 탭 생성을 막기 위해 문서당 자동 열기를 5회로 제한하고 이후 요청은 수동 허용 대상으로 보류합니다. 주소 없는 팝업이나 `about:blank`에 스크립트로 내용을 쓰는 창은 지원하지 않습니다.

기존 2026-11-17 배포 ZIP에는 이 팝업 기능이 포함되어 있지 않습니다. 최신 소스를 빌드해야 합니다.

## 복사해서 실행하기

빌드된 `PlainBrowser-0.1.0-windows-x64.zip`이 있다면 다음과 같이 실행합니다. 저장소에 배포 ZIP이 포함되어 있다고 가정하지는 않습니다.

1. Windows x64 PC의 쓰기 가능한 폴더에 ZIP 전체를 압축 해제합니다.
2. 함께 제공된 DLL과 `licenses` 폴더를 유지합니다.
3. `plain-browser.exe`를 실행합니다.

Rust, Cargo, Visual Studio는 실행 PC에 설치할 필요가 없습니다. **Microsoft Edge WebView2 Runtime은 필요합니다.** 런타임이 없는 PC에서는 [Microsoft 공식 다운로드](https://developer.microsoft.com/microsoft-edge/webview2/)에서 Evergreen Runtime x64를 설치하세요.

Windows 11 환경에서 개발·검증했습니다. 다른 PC에서의 실제 호환성 확인은 진행 중입니다. 배포본은 코드 서명이 없는 시험 버전이며, 시스템 설정이나 보안 정책을 자동으로 변경하지 않습니다.

### 사용자 데이터

첫 실행 시 실행 파일 옆 `data/webview2` 폴더에 쿠키·캐시 등의 데이터를 생성합니다. 쓰기 권한이 없는 폴더에는 배치하지 마세요.

- 배포 ZIP에는 개발 PC의 사용자 데이터를 포함하지 않습니다.
- 개인 데이터를 옮기지 않으려면 브라우저를 종료한 뒤 `data` 폴더를 제외하고 복사하세요.
- 시크릿 모드가 아니며, 폴더를 복사할 때 사용자 데이터가 함께 포함될 수 있습니다.

## 개발 환경과 빌드

개발 환경:

- Windows x64
- Rust / Cargo — 현재 검증 버전 `1.100.0`, Rust 2024 edition
- MSVC C++ 빌드 도구와 Windows SDK
- Microsoft Edge WebView2 Runtime

MSVC 빌드 도구는 Visual Studio 또는 Visual Studio Build Tools의 C++ 데스크톱 개발 구성으로 준비할 수 있습니다. 개발 도구는 소스 빌드에 필요하며, 배포본 실행에는 필요하지 않습니다.

프로젝트 폴더에서 실행합니다.

```powershell
# 개발 실행
cargo run --locked

# 최적화 빌드
cargo build --release --locked
```

결과 실행 파일은 `target/release/plain-browser.exe`입니다. 이 파일만 복사할 경우 대상 PC의 VC 런타임 설치 상태에 따라 보조 DLL이 필요할 수 있으므로, 복사 배포에는 필요한 DLL을 함께 묶은 ZIP을 사용하는 편이 좋습니다.

### 배포 ZIP 만들기

저장소의 [`tools/package-portable.ps1`](tools/package-portable.ps1)은 Release 실행 파일, VC 런타임 DLL, 사용 안내, 체크섬, 의존성 라이선스와 adblock-rust 원본 소스를 묶습니다. 먼저 Release 빌드와 의존성 메타데이터를 준비합니다.

```powershell
New-Item -ItemType Directory -Force work | Out-Null
cargo build --release --locked
cargo metadata --locked --format-version 1 |
    Out-File work/portable-metadata.json -Encoding utf8

# 경로는 설치된 Visual Studio의 MSVC 재배포 파일 위치로 바꿉니다.
.\tools\package-portable.ps1 -CrtDirectory 'C:\path\to\x64\Microsoft.VC143.CRT'
```

결과는 `dist/PlainBrowser-0.1.0-windows-x64/`와 같은 이름의 ZIP입니다. 기존 결과 폴더가 있으면 덮어쓰지 않고 중단합니다. 버전과 안내 날짜는 현재 스크립트에 지정되어 있으므로 새 배포 시 갱신해야 합니다.

## 검증

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings

# 별도 테스트 프로필을 사용하는 실제 WebView2 자동 검증
cargo run --locked --example check_tabs -- --check-tabs
cargo run --locked --example check_tabs -- --check-interactions
```

단위 테스트 15개와 WebView2 기반의 광고 요청 차단·링크 이동·탭 동작 검증을 수행했습니다. 배포 ZIP은 압축 해제 후 실행 파일 해시 일치와 개인 데이터 미포함을 확인했습니다. 탭 클릭 시 세로 스크롤을 방지하는 스타일 수정은 빌드를 완료했으며 실제 화면의 최종 사용자 확인이 남아 있습니다.

## 현재 한계와 다음 작업

- EasyList 자동 갱신과 실행 중 외부 목록 읽기를 지원하지 않습니다.
- 탭 순서와 열린 탭을 다음 실행에 복원하는 기능은 없습니다.
- 즐겨찾기, 검색어 검색, 본문 전용 보기, 시크릿 모드는 아직 구현하지 않았습니다.
- 설치 프로그램, 기본 브라우저 등록, 코드 서명은 제공하지 않습니다.
- macOS·Linux용 빌드와 검증은 진행하지 않았습니다.

## 프로젝트 구성

```text
src/            브라우저 동작, 광고 차단, 탭 및 내장 화면
assets/         PB 아이콘과 로고
filters/        EasyList 원본, 출처 및 라이선스
examples/       WebView2 동작 검증 도구
tools/          아이콘 생성 및 배포 패키징 도구
docs/index.html 프로젝트 방향, 작업 기록, 검증 결과
```

자세한 진행 기록은 [`docs/index.html`](docs/index.html)을 브라우저에서 열어 확인할 수 있습니다. 저장소에 파일이 있다고 해서 GitHub Pages로 게시된 것은 아닙니다.

## 출처와 라이선스

- [adblock-rust](https://github.com/brave/adblock-rust): MPL-2.0. [`라이선스 사본`](filters/adblock-rust-MPL-2.0.txt)
- [EasyList](https://easylist.to/): 이 프로젝트의 스냅샷은 CC-BY-SA-3.0 조건으로 제공합니다. [`출처 안내`](filters/NOTICE.txt), [`라이선스 사본`](filters/CC-BY-SA-3.0.txt)
- 그 밖의 의존성에는 각 프로젝트의 라이선스가 적용됩니다. 배포 패키지의 `licenses` 폴더에 관련 고지를 포함합니다.

Plain Browser 자체 코드의 배포 라이선스는 아직 정하지 않았습니다. 위 의존성 라이선스가 프로젝트 자체 코드의 사용 허가를 대신하지는 않습니다.
