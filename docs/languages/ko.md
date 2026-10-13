# PDF Editor Offline

**소스의 새 기능:** 기존 페이지에 PNG/JPEG를 삽입하고 이동, 크기 조절, 90도 단위 회전과 실행 취소·다시 실행을 지원합니다. [이미지 가이드](../IMAGE_OVERLAYS_RUST.md)(영문). 배포된 CLI `4.0.0-preview.3`에는 포함되지 않으며 기존 이미지 교체는 아직 구현되지 않았습니다.

**개발 진행 상황:** [최근 변경 사항](https://github.com/OthmaneBlial/pdf-editor-offline/commits/main/) · [네이티브 테스트](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [로드맵](../../ROADMAP.md). 작업 흐름을 사용해 보고 개인정보가 없는 예제 PDF로 개선에 참여해 주세요. 유용한 도구라면 [프로젝트에 별을 주세요](https://github.com/OthmaneBlial/pdf-editor-offline).

**소스 코드의 새 기능:** 드래그와 크기 조절 핸들로 새 텍스트 배치, PDF를 새 탭으로 분할, 비밀번호 대화상자로 보호된 문서 열기. 암호화된 문서는 읽기 전용이며 복구 기록에 복호화된 내용이나 비밀번호를 저장하지 않습니다. 배포된 CLI는 여전히 `4.0.0-preview.3`입니다.

[Encrypted viewing](../ENCRYPTED_VIEWING_RUST.md) · [Canvas text placement](../TEXT_OVERLAYS_RUST.md) · [Split workflow](../../apps/desktop/README.md)

**Rust로 만든 오프라인 PDF 편집기. 파일은 내 컴퓨터에 그대로 남습니다.**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

**현재 소스의 새 기능:** 지원되는 PDF를 새 복사본으로 합치고, 페이지에 Unicode 텍스트를 추가하고, 기본 메모를 관리할 수 있습니다. 실행 취소와 다시 실행도 지원합니다. 사용하려면 소스에서 빌드하세요. 공개된 CLI는 아직 `4.0.0-preview.3`입니다. [텍스트 예제](../TEXT_OVERLAYS_RUST.md) · [메모 안내](../NOTES_RUST.md) (영문 가이드). 텍스트 추가나 일반적인 메모 삭제는 민감한 데이터를 안전하게 지우지 않습니다.

PDF를 열고 필요한 페이지를 찾은 뒤 문서를 정리하고 새 복사본으로 저장하세요. PDF Editor Offline은 무료 오픈 소스 데스크톱 앱과 명령줄 도구입니다. 계정, 구독, 문서 업로드가 필요하지 않습니다.

현재는 네이티브 Rust 미리보기 버전입니다. 데스크톱 앱을 사용하려면 소스에서 빌드하세요. 대상 플랫폼은 macOS, Windows, Linux이며 실제 데스크톱 조작 검증은 주로 macOS에서 진행했습니다. 데스크톱 설치 프로그램과 완전한 PDF 편집 기능은 개발 중입니다. [네이티브 CLI 사전 릴리스 4.0.0-preview.3](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.3)이 공개되어 있으며, [이전 데스크톱 릴리스 모음](https://github.com/OthmaneBlial/pdf-editor-offline/releases)은 과거 빌드입니다.

![macOS에서 실행 중인 실제 네이티브 앱: 로컬 검색, PDF 페이지, 문서 속성](../assets/screenshots/native/editorial-light-2026-10-12.png)

## 현재 가능한 작업

| 작업 | 지원 범위 |
| --- | --- |
| 읽기와 검색 | 탭, 확대·축소, 보이는 페이지의 미리보기, 텍스트 보기, 문자열 검색, 밝은/어두운 테마. CLI에서는 문서 정보, 텍스트 추출, 제한적인 텍스트·크기 비교. |
| 페이지 정리 | 앱에서 이동, 순서 뒤집기, 회전, 빈 페이지 추가, 자르기, 실행 취소/다시 실행. CLI에서 제한된 추출·삭제 및 단순한 카탈로그 구조의 병합. |
| 문서 속성 | Unicode를 지원하는 표준 필드 6개 읽기·수정: Title, Author, Subject, Keywords, Creator, Producer. |
| 이미지와 주석 | 지원하는 PNG/JPEG로 PDF 페이지 생성. CLI에서 한 페이지를 PNG로 내보내고 Text 메모 추가·조회·제한적 수정·삭제. |
| 검사와 저장 | CLI에서 동작, 첨부, 양식, 레이어 표시를 읽기 전용으로 검사. 무손실 스트림 압축 및 검증된 새 복사본 저장, 기존 파일 덮어쓰기 방지. |

병합, 페이지 부분 선택, 메모 수정, 이미지 변환에는 명확한 제한이 있습니다. 지원하지 않는 구조는 거부합니다. [CLI 안내](../../apps/cli/README.md)를 참고하세요. 압축해도 파일이 작아진다는 보장은 없습니다. 자르기는 내용을 숨길 뿐이며 메타데이터나 메모를 제거해도 이전 값이 남을 수 있습니다. 안전한 데이터 삭제가 아닙니다. OCR, 고급 텍스트 필드(글자별 칸 또는 서식 있는 텍스트) 입력, 디지털 서명, 자유로운 본문 편집, 일반적인 문서 변환은 구현되지 않았습니다. 지원하지 않는 내용이 렌더링에서 빠질 수 있으므로 내보낸 문서를 확인하세요. 서명된 파일과 정상적으로 복호화한 입력은 읽기 전용입니다.

현재 소스는 AcroForm 필드를 검색하고 기존 모양을 유지하면서 지원되는 체크박스와 라디오 버튼을 변경합니다. [사용 예와 제한](../FORMS_RUST.md)을 참고하세요. 공개된 npm CLI `4.0.0-preview.3`에서는 이러한 버튼 필드를 변경할 수 있습니다.

현재 소스에서만 지원되는 한 줄 및 여러 줄 텍스트 필드에 Unicode 텍스트를 입력할 수 있습니다. 전체 값을 표현할 수 있는 TrueType 또는 CID CFF1 글꼴을 직접 선택해야 합니다. 여러 줄 필드는 명시적인 줄바꿈을 유지하며 필드 너비에 맞춰 줄을 나눕니다. 크기 조정에는 제한이 있으며, 넘치는 텍스트, 누락된 글리프, 지원하지 않는 레이아웃은 텍스트를 잘라내지 않고 거부합니다.

현재 소스에서는 지원되는 드롭다운과 단일·다중 선택 목록을 0부터 시작하는 옵션 인덱스와 직접 선택한 글꼴로 입력할 수 있습니다. 텍스트 및 선택 필드 입력은 공개된 CLI `4.0.0-preview.3`에 포함되지 않습니다. 자유 입력 드롭다운과 지원하지 않는 레이아웃은 거부합니다.

현재 UI는 영어입니다. 이 문서 번역이 GUI 현지화나 완전한 오른쪽에서 왼쪽 방향 레이아웃 지원을 뜻하지는 않습니다. 오프라인 글꼴에는 검증된 중국어·일본어·한국어 글리프가 포함되지만 모든 문자 체계를 지원하지는 않습니다.

## 네이티브 CLI 설치 (선택)

npm 설치와 런처에는 Node.js 22 이상이 필요합니다. 독립적인 CLI 실행 파일은 Node.js가 필요하지 않습니다. 데스크톱 앱은 계속 Rust 소스에서 빌드해야 합니다.

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

### 내 PDF로 사용해 보기

`input.pdf`를 로컬 파일로 바꾸세요. 첫 번째 명령은 정보를 읽고, 두 번째는 파일을 쓰지 않고 회전을 검증하며, 세 번째는 새 복사본을 만듭니다. 출력 파일에는 아직 존재하지 않는 이름을 지정하세요.

```sh
pdf-editor-offline info input.pdf --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --json
```

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

Windows에서는 `target\release\pdf-editor-offline-native.exe`를 실행합니다. 페이지 번호는 1부터 시작합니다. `--dry-run`은 파일을 쓰지 않고 검증합니다. 기존 파일과 심볼릭 링크를 덮어쓰지 않으므로 새 출력 경로를 선택하세요.

## 로컬 처리와 확인 가능한 결과

PDF 작업은 내 컴퓨터에서 수행되며 원격 렌더링이나 클라우드 변환 서비스를 쓰지 않습니다. 그렇다고 모든 PDF의 안전성이나 숨은 정보의 완전한 제거를 증명하지는 않습니다. [CLI 검증 기록](../RUST_BENCHMARKS.md)은 qpdf/Poppler를 사용한 44개 작업 검사, 실제 프로세스 시간, 해시를 담고 있으며 생성한 1,000페이지 PDF도 포함합니다. 모든 문서의 재현 정확도, 데스크톱 응답성, 보편적인 속도 향상을 입증하는 결과는 아닙니다.

합성 PDF로 재현 가능한 문제, 렌더링 비교, 접근성 테스트, 번역 수정, 작은 Rust 변경을 환영합니다. 공개 보고에는 개인 문서를 첨부하지 마세요.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../assets/screenshots/native/)

로컬 PDF 편집기가 유용하다면 저장소에 별표를 눌러 다른 사람이 찾는 데 도움을 주세요.

MIT 라이선스이며 글꼴과 의존성은 각자의 고지 사항을 유지합니다.
