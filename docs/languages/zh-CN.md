# PDF Editor Offline

**用 Rust 编写的离线 PDF 编辑器。文件始终留在你的电脑上。**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

打开 PDF，找到需要的页面，调整文档，然后保存一份新副本。PDF Editor Offline 提供免费、开源的桌面应用和命令行工具，无需账号、订阅或上传文档。

目前是原生 Rust 预览版。请从源码构建以体验当前应用。目标平台为 macOS、Windows 和 Linux；实际桌面交互验证主要在 macOS 上进行。原生安装包和完整 PDF 编辑功能仍在开发。[历史发行版本](https://github.com/OthmaneBlial/pdf-editor-offline/releases)属于早期构建。

![macOS 上真实的原生应用窗口，显示本地搜索、PDF 页面和文档属性](../../screenshots/native/editorial-light-2026-10-12.png)

## 当前可用功能

| 任务 | 可用功能 |
| --- | --- |
| 阅读与搜索 | 桌面提供标签页、缩放、可见页面缩略图、文本视图、字面搜索和明暗主题；CLI 提供信息查询、文本提取和有限的文本/几何比较。 |
| 页面整理 | 桌面支持移动、反转、旋转、插入空白页、裁剪以及撤销/重做；CLI 支持受限制的页面提取/删除和简单目录结构的合并。 |
| 文档属性 | 读取和编辑六个标准字段，支持 Unicode：Title、Author、Subject、Keywords、Creator、Producer。 |
| 图像与注释 | 将支持的 PNG/JPEG 转为 PDF 页面；CLI 可导出单页 PNG，并添加、列出、有限编辑和删除 Text 便笺。 |
| 检查与保存 | CLI 只读检查动作、附件、表单和图层标记，进行无损流压缩；保存经验证的新副本，不覆盖已有文件。 |

合并、页面子集、便笺编辑和图像转换均有明确限制。不支持的结构会被拒绝，详见 [CLI 指南](../../apps/cli/README.md)。压缩不保证文件变小。裁剪只是隐藏内容；删除元数据或便笺可能留下旧值，不等于安全擦除。预览版尚不支持 OCR、表单填写、数字签名、任意页面文本编辑或通用文档转换。渲染器可能遗漏不支持的内容，请检查导出的文档。已签名及成功解密的输入仅可读取。

界面目前为英语。这些说明文档翻译不代表 GUI 已本地化，也不代表完整支持从右到左的布局。离线字体包含已测试的中文、日文和韩文字形，但不能覆盖所有文字系统。

## 构建与运行

安装 Rust 1.95+ 以及平台所需的原生构建库。首次下载依赖需要联网。Linux 系统包请参考[原生 CI 配置](../../.github/workflows/rust.yml)。

```sh
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
cargo build --release --locked -p pdf-editor-offline-native -p pdf-editor-offline-cli -p pdf-worker
./target/release/pdf-editor-offline-native
```

```sh
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --json
./target/release/pdf-editor-offline capabilities --json
```

请将 worker 可执行文件与桌面程序放在同一目录。Windows 使用 `target\release\pdf-editor-offline-native.exe`。页码从 1 开始。`--dry-run` 只验证，不写文件。请选择新的输出路径；已有文件和符号链接不会被覆盖。

## 本地处理，可核对结果

PDF 操作在本机完成，不使用远程渲染或云转换服务。这并不能证明每份 PDF 都安全，也不能证明所有隐藏信息已被清除。[CLI 验证记录](../RUST_BENCHMARKS.md)包含 44 项操作检查、qpdf/Poppler 独立验证、实际进程耗时和哈希，其中包括一份生成的 1,000 页 PDF。这些结果不证明所有文档都能完整还原、桌面响应流畅或普遍提速。

欢迎提供可复现的合成 PDF 测试案例、渲染对比、无障碍测试、翻译修正和小型 Rust 改动。请勿在公开报告中附上私人文档。

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

如果你需要在本地工作的 PDF 编辑器，欢迎给仓库点一颗星，帮助更多人找到它。

采用 MIT 许可证；字体和依赖保留各自的许可声明。

