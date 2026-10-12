# PDF Editor Offline

**Rust로 만든 오프라인 PDF 편집기. 파일은 내 컴퓨터에 그대로 남습니다.**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

PDF를 열고 필요한 페이지를 찾은 뒤 문서를 정리하고 새 복사본으로 저장하세요. PDF Editor Offline은 무료 오픈 소스 데스크톱 앱과 명령줄 도구입니다. 계정, 구독, 문서 업로드가 필요하지 않습니다.

현재는 네이티브 Rust 미리보기 버전입니다. 최신 앱을 사용하려면 소스에서 빌드하세요. 대상 플랫폼은 macOS, Windows, Linux이며 실제 데스크톱 조작 검증은 주로 macOS에서 진행했습니다. 네이티브 설치 프로그램과 완전한 PDF 편집 기능은 개발 중입니다. [이전 릴리스 모음](https://github.com/OthmaneBlial/pdf-editor-offline/releases)은 과거 빌드입니다.

![macOS에서 실행 중인 실제 네이티브 앱: 로컬 검색, PDF 페이지, 문서 속성](../../screenshots/native/editorial-light-2026-10-12.png)

## 현재 가능한 작업

| 작업 | 지원 범위 |
| --- | --- |
| 읽기와 검색 | 탭, 확대·축소, 보이는 페이지의 미리보기, 텍스트 보기, 문자열 검색, 밝은/어두운 테마. CLI에서는 문서 정보, 텍스트 추출, 제한적인 텍스트·크기 비교. |
| 페이지 정리 | 앱에서 이동, 순서 뒤집기, 회전, 빈 페이지 추가, 자르기, 실행 취소/다시 실행. CLI에서 제한된 추출·삭제 및 단순한 카탈로그 구조의 병합. |
| 문서 속성 | Unicode를 지원하는 표준 필드 6개 읽기·수정: Title, Author, Subject, Keywords, Creator, Producer. |
| 이미지와 주석 | 지원하는 PNG/JPEG로 PDF 페이지 생성. CLI에서 한 페이지를 PNG로 내보내고 Text 메모 추가·조회·제한적 수정·삭제. |
| 검사와 저장 | CLI에서 동작, 첨부, 양식, 레이어 표시를 읽기 전용으로 검사. 무손실 스트림 압축 및 검증된 새 복사본 저장, 기존 파일 덮어쓰기 방지. |

병합, 페이지 부분 선택, 메모 수정, 이미지 변환에는 명확한 제한이 있습니다. 지원하지 않는 구조는 거부합니다. [CLI 안내](../../apps/cli/README.md)를 참고하세요. 압축해도 파일이 작아진다는 보장은 없습니다. 자르기는 내용을 숨길 뿐이며 메타데이터나 메모를 제거해도 이전 값이 남을 수 있습니다. 안전한 데이터 삭제가 아닙니다. OCR, 양식 작성, 디지털 서명, 자유로운 본문 편집, 일반적인 문서 변환은 구현되지 않았습니다. 지원하지 않는 내용이 렌더링에서 빠질 수 있으므로 내보낸 문서를 확인하세요. 서명된 파일과 정상적으로 복호화한 입력은 읽기 전용입니다.

현재 UI는 영어입니다. 이 문서 번역이 GUI 현지화나 완전한 오른쪽에서 왼쪽 방향 레이아웃 지원을 뜻하지는 않습니다. 오프라인 글꼴에는 검증된 중국어·일본어·한국어 글리프가 포함되지만 모든 문자 체계를 지원하지는 않습니다.

## 빌드와 실행

Rust 1.95 이상과 플랫폼의 네이티브 빌드 라이브러리를 설치하세요. 의존성을 처음 내려받을 때는 인터넷 연결이 필요합니다. Linux 시스템 패키지는 [네이티브 CI 설정](../../.github/workflows/rust.yml)을 참고하세요.

```sh
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
cargo build --release --locked -p pdf-editor-offline-native -p pdf-editor-offline-cli
./target/release/pdf-editor-offline-native
```

```sh
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --json
./target/release/pdf-editor-offline capabilities --json
```

worker 실행 파일을 데스크톱 실행 파일과 같은 디렉터리에 두세요. Windows에서는 `target\release\pdf-editor-offline-native.exe`를 실행합니다. 페이지 번호는 1부터 시작합니다. `--dry-run`은 파일을 쓰지 않고 검증합니다. 기존 파일과 심볼릭 링크를 덮어쓰지 않으므로 새 출력 경로를 선택하세요.

## 로컬 처리와 확인 가능한 결과

PDF 작업은 내 컴퓨터에서 수행되며 원격 렌더링이나 클라우드 변환 서비스를 쓰지 않습니다. 그렇다고 모든 PDF의 안전성이나 숨은 정보의 완전한 제거를 증명하지는 않습니다. [CLI 검증 기록](../RUST_BENCHMARKS.md)은 qpdf/Poppler를 사용한 44개 작업 검사, 실제 프로세스 시간, 해시를 담고 있으며 생성한 1,000페이지 PDF도 포함합니다. 모든 문서의 재현 정확도, 데스크톱 응답성, 보편적인 속도 향상을 입증하는 결과는 아닙니다.

합성 PDF로 재현 가능한 문제, 렌더링 비교, 접근성 테스트, 번역 수정, 작은 Rust 변경을 환영합니다. 공개 보고에는 개인 문서를 첨부하지 마세요.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

로컬 PDF 편집기가 유용하다면 저장소에 별표를 눌러 다른 사람이 찾는 데 도움을 주세요.

MIT 라이선스이며 글꼴과 의존성은 각자의 고지 사항을 유지합니다.

