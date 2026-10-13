# PDF Editor Offline

**Un editor de PDF sin conexión, escrito en Rust. Tus archivos se quedan en tu ordenador.**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

Abre un PDF, encuentra la página que necesitas, organiza el documento y guarda una copia nueva. PDF Editor Offline es una aplicación de escritorio y una herramienta de línea de comandos gratuitas y de código abierto. No necesitas cuenta, suscripción ni subir tus documentos.

Esta es la versión preliminar nativa en Rust. Compila la aplicación de escritorio desde el código fuente para probarla. macOS, Windows y Linux son las plataformas objetivo; la interacción real del escritorio se ha comprobado principalmente en macOS. Los instaladores de escritorio y la edición completa siguen en desarrollo. La [CLI nativa preliminar 4.0.0-preview.1](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.1) está disponible; el [archivo de versiones de escritorio](https://github.com/OthmaneBlial/pdf-editor-offline/releases) contiene versiones anteriores.

![Ventana real de la aplicación nativa en macOS con búsqueda local, página PDF y propiedades](../../screenshots/native/editorial-light-2026-10-12.png)

## Qué funciona

| Tarea | Disponible |
| --- | --- |
| Leer y buscar | Pestañas, zoom, miniaturas visibles, texto, búsqueda literal y temas claro/oscuro; información, extracción y comparación limitada en CLI. |
| Organizar | Mover, invertir y girar páginas, añadir páginas en blanco, recortar, deshacer/rehacer en la aplicación; extracción/eliminación protegidas y unión de catálogos simples en CLI. |
| Propiedades | Leer y editar seis campos estándar con Unicode: Title, Author, Subject, Keywords, Creator y Producer. |
| Imágenes y comentarios | Crear PDF desde PNG/JPEG compatibles; exportar una página a PNG y añadir/listar/editar/eliminar notas Text bajo restricciones en CLI. |
| Inspeccionar y guardar | Inventario de marcadores de acciones, adjuntos, formularios y capas de solo lectura en CLI; compresión de flujos sin pérdida y copias nuevas verificadas sin sobrescribir. |

La unión, los subconjuntos de páginas, las notas y la conversión de imágenes tienen restricciones explícitas. Las estructuras no compatibles se rechazan; consulta la [guía CLI](../../apps/cli/README.md). La compresión no garantiza un archivo menor. Recortar oculta contenido; eliminar metadatos o notas puede dejar valores antiguos. No es un borrado seguro. OCR, campos de selección y de texto avanzado, firma digital, edición libre del texto y conversión general no están disponibles. El renderizador puede omitir contenido no compatible. Revisa los documentos exportados. Los archivos firmados y los descifrados correctamente son de solo lectura.

El código actual detecta campos AcroForm y rellena casillas y botones de opción compatibles, conservando sus apariencias existentes. [Ejemplos y límites](../FORMS_RUST.md). Esta función todavía no está en el paquete npm publicado.

Los campos de texto de una sola línea compatibles también pueden rellenarse con una fuente TrueType o CFF1 CID explícita que cubra todo el valor. Se rechazan los glifos ausentes y los diseños no compatibles.

La interfaz está actualmente en inglés. Estas traducciones no implican una interfaz localizada ni compatibilidad completa con diseños de derecha a izquierda. Las fuentes locales incluyen caracteres chinos, japoneses y coreanos probados, pero no cubren todas las escrituras.

## Instalar la CLI nativa (opcional)

La instalación con npm y su lanzador requieren Node.js 22+. El binario independiente de la CLI no necesita Node.js. La aplicación de escritorio se sigue compilando desde el código fuente Rust.

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

### Prueba con tu PDF

Sustituye `input.pdf` por un archivo local. El primer comando lee sus datos; el segundo comprueba la rotación sin escribir; el tercero crea una copia nueva. Elige un nombre de salida que no exista.

```sh
pdf-editor-offline info input.pdf --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --json
```

## Compilar y ejecutar

Instala Rust 1.95+ y las bibliotecas nativas de tu plataforma. La primera descarga de dependencias requiere conexión. En Linux, consulta los [paquetes de la CI nativa](../../.github/workflows/rust.yml).

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

En Windows ejecuta `target\release\pdf-editor-offline-native.exe`. Las páginas empiezan en 1. `--dry-run` valida sin escribir. Elige una ruta de salida nueva: nunca se sobrescriben archivos ni enlaces simbólicos existentes.

## Local y comprobable

Las operaciones se ejecutan en tu equipo, sin servicios remotos de renderizado o conversión. Esto no demuestra que cada PDF sea seguro ni que se hayan eliminado todos los datos ocultos. Las [pruebas CLI](../RUST_BENCHMARKS.md) registran 44 comprobaciones con qpdf/Poppler, tiempos de proceso y hashes, incluido un PDF sintético de 1.000 páginas. No garantizan fidelidad universal, respuesta del escritorio ni una mejora de velocidad general.

Puedes aportar casos reproducibles con PDF sintéticos, comparaciones de renderizado, pruebas de accesibilidad, correcciones de traducción y pequeños cambios en Rust. No adjuntes documentos privados a informes públicos.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

Si te sirve un editor de PDF local, deja una estrella en el repositorio para que otras personas lo encuentren.

Licencia MIT; las fuentes y dependencias conservan sus propios avisos.
