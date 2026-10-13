# PDF Editor Offline

**Um editor de PDF offline feito em Rust. Seus arquivos ficam no seu computador.**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

Abra um PDF, encontre a página desejada, organize o documento e salve uma nova cópia. PDF Editor Offline é um aplicativo de desktop e uma ferramenta de linha de comando gratuitos e de código aberto. Não exige conta, assinatura ou envio de documentos.

Esta é a prévia nativa em Rust. Compile o aplicativo de desktop a partir do código-fonte para testá-lo. macOS, Windows e Linux são as plataformas previstas; a interação real no desktop foi verificada principalmente no macOS. Instaladores de desktop e edição completa ainda estão em desenvolvimento. A [prévia da CLI nativa 4.0.0-preview.1](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.1) está disponível; o [arquivo de versões de desktop](https://github.com/OthmaneBlial/pdf-editor-offline/releases) contém versões anteriores.

![Janela real do aplicativo nativo no macOS com busca local, página PDF e propriedades](../../screenshots/native/editorial-light-2026-10-12.png)

## O que funciona

| Tarefa | Disponível |
| --- | --- |
| Ler e buscar | Abas, zoom, miniaturas visíveis, texto, busca literal e temas claro/escuro; informações, extração e comparação limitada pela CLI. |
| Organizar | Mover, inverter e girar páginas, inserir páginas em branco, recortar, desfazer/refazer no aplicativo; extração/exclusão protegidas e união de catálogos simples pela CLI. |
| Propriedades | Ler e editar seis campos padrão com Unicode: Title, Author, Subject, Keywords, Creator e Producer. |
| Imagens e comentários | Criar PDF com PNG/JPEG compatíveis; exportar uma página como PNG e adicionar/listar/editar/remover notas Text com restrições pela CLI. |
| Inspecionar e salvar | Inventário de indicadores de ações, anexos, formulários e camadas, somente leitura pela CLI; compressão de fluxos sem perdas e novas cópias verificadas sem sobrescrever. |

União, subconjuntos de páginas, notas e conversão de imagens têm restrições explícitas. Estruturas não suportadas são recusadas; veja o [guia da CLI](../../apps/cli/README.md). A compressão não garante um arquivo menor. Recortar esconde conteúdo; remover metadados ou notas pode deixar valores antigos. Isso não é apagamento seguro. OCR, preenchimento de campos de texto e de seleção, assinatura digital, edição livre do texto e conversão geral não estão disponíveis. O renderizador pode omitir conteúdo não suportado. Confira os documentos exportados. Arquivos assinados e os descriptografados com sucesso são somente leitura.

O código atual identifica campos AcroForm e preenche caixas de seleção e botões de opção compatíveis, preservando suas aparências existentes. [Exemplos e limites](../FORMS_RUST.md). A função ainda não está no pacote npm publicado.

A interface está em inglês. Estas traduções não significam que o aplicativo esteja localizado ou que tenha suporte completo a layouts da direita para a esquerda. As fontes offline incluem caracteres chineses, japoneses e coreanos testados, sem cobrir todos os sistemas de escrita.

## Instalar a CLI nativa (opcional)

A instalação pelo npm e seu inicializador precisam de Node.js 22+. O executável independente da CLI não precisa de Node.js. O aplicativo de desktop continua sendo compilado a partir do código-fonte Rust.

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

### Experimente com seu PDF

Substitua `input.pdf` por um arquivo local. O primeiro comando lê as informações; o segundo verifica a rotação sem gravar; o terceiro cria uma nova cópia. Escolha um nome de saída que ainda não exista.

```sh
pdf-editor-offline info input.pdf --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --json
```

## Compilar e executar

Instale Rust 1.95+ e as bibliotecas nativas da sua plataforma. O primeiro download de dependências precisa de conexão. No Linux, consulte os [pacotes da CI nativa](../../.github/workflows/rust.yml).

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

No Windows, execute `target\release\pdf-editor-offline-native.exe`. As páginas começam em 1. `--dry-run` valida sem gravar. Escolha um novo caminho de saída: arquivos e links simbólicos existentes nunca são sobrescritos.

## Local e verificável

As operações acontecem no seu computador, sem serviço remoto de renderização ou conversão. Isso não comprova a segurança de todo PDF nem a remoção de todos os dados ocultos. As [evidências da CLI](../RUST_BENCHMARKS.md) registram 44 verificações com qpdf/Poppler, tempos de processo e hashes, incluindo um PDF sintético de 1.000 páginas. Não demonstram fidelidade universal, responsividade do desktop ou ganho de velocidade geral.

Contribuições úteis incluem casos reproduzíveis com PDFs sintéticos, comparações de renderização, testes de acessibilidade, correções de tradução e pequenas alterações em Rust. Não inclua documentos privados em relatos públicos.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

Se um editor de PDF local é útil para você, dê uma estrela ao repositório para ajudar outras pessoas a encontrá-lo.

Licença MIT; fontes e dependências mantêm seus próprios avisos.
