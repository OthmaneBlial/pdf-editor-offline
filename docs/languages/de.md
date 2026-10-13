# PDF Editor Offline

**Ein Offline-PDF-Editor in Rust. Deine Dateien bleiben auf deinem Rechner.**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

Öffne ein PDF, finde die gewünschte Seite, ordne das Dokument und speichere eine neue Kopie. PDF Editor Offline ist eine kostenlose, quelloffene Desktop-App mit Kommandozeilenwerkzeug. Kein Konto, Abo oder Hochladen von Dokumenten ist erforderlich.

Dies ist die native Rust-Vorschau. Baue die Desktop-App aus dem Quellcode, um sie zu testen. Zielplattformen sind macOS, Windows und Linux; die tatsächliche Desktop-Bedienung wurde vor allem unter macOS geprüft. Desktop-Installationspakete und vollständige PDF-Bearbeitung sind noch in Arbeit. Die [native CLI-Vorschau 4.0.0-preview.1](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.1) ist verfügbar; das [Archiv der Desktop-Versionen](https://github.com/OthmaneBlial/pdf-editor-offline/releases) enthält frühere Builds.

![Echtes natives App-Fenster unter macOS mit lokaler Suche, PDF-Seite und Dokumenteigenschaften](../../screenshots/native/editorial-light-2026-10-12.png)

## Was bereits funktioniert

| Aufgabe | Verfügbar |
| --- | --- |
| Lesen und suchen | Tabs, Zoom, sichtbare Seitenminiaturen, Textansicht, wörtliche Suche und helle/dunkle Darstellung; Informationen, Textextraktion und begrenzter Vergleich per CLI. |
| Seiten ordnen | Verschieben, Reihenfolge umkehren, drehen, leere Seiten einfügen, zuschneiden und rückgängig/wiederholen in der App; geprüftes Extrahieren/Löschen und Zusammenführen einfacher Kataloge per CLI. |
| Eigenschaften | Sechs Standardfelder mit Unicode lesen und bearbeiten: Title, Author, Subject, Keywords, Creator und Producer. |
| Bilder und Kommentare | PDF-Seiten aus unterstützten PNG/JPEG-Dateien; eine Seite als PNG exportieren und einfache Text-Notizen per CLI hinzufügen/auflisten/eingeschränkt bearbeiten/entfernen. |
| Prüfen und speichern | Aktions-, Anhang-, Formular- und Ebenenmerkmale per CLI nur lesend erfassen; Streams verlustfrei komprimieren und geprüfte neue Kopien ohne Überschreiben speichern. |

Zusammenführen, Seitenauswahl, Notizen und Bildkonvertierung haben klare Grenzen. Nicht unterstützte Strukturen werden abgewiesen; siehe [CLI-Anleitung](../../apps/cli/README.md). Komprimierung garantiert keine kleinere Datei. Zuschneiden versteckt Inhalte; beim Entfernen von Metadaten oder Notizen können alte Werte erhalten bleiben. Das ist kein sicheres Löschen. OCR, Auswahlfelder und erweiterte Textfelder, digitale Signaturen, freie Textbearbeitung und allgemeine Dokumentkonvertierung fehlen noch. Nicht unterstützte Inhalte können beim Rendern fehlen. Prüfe exportierte Dokumente. Signierte und erfolgreich entschlüsselte Dateien sind nur lesbar.

Der aktuelle Quellcode erkennt AcroForm-Felder und füllt unterstützte Kontrollkästchen und Optionsfelder mit vorhandenen Darstellungen. [Beispiele und Grenzen](../FORMS_RUST.md). Diese Funktion ist noch nicht im veröffentlichten npm-Paket enthalten.

Unterstützte einzeilige Textfelder können mit einer ausdrücklich gewählten TrueType- oder CID-CFF1-Schrift ausgefüllt werden, die den gesamten Wert abdeckt. Fehlende Glyphen und nicht unterstützte Layouts werden abgewiesen.

Die Oberfläche ist derzeit auf Englisch. Diese Übersetzungen bedeuten keine lokalisierte GUI oder vollständige Unterstützung für Rechts-nach-links-Layouts. Lokale Schriften enthalten getestete chinesische, japanische und koreanische Zeichen, decken aber nicht jede Schrift ab.

## Native CLI installieren (optional)

Die npm-Installation und ihr Launcher benötigen Node.js 22+. Die eigenständige CLI-Binärdatei braucht kein Node.js. Die Desktop-App wird weiterhin aus dem Rust-Quellcode gebaut.

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

### Mit deiner PDF ausprobieren

Ersetze `input.pdf` durch eine lokale Datei. Der erste Befehl liest die Informationen; der zweite prüft die Drehung ohne zu schreiben; der dritte erstellt eine neue Kopie. Wähle einen noch nicht vorhandenen Ausgabenamen.

```sh
pdf-editor-offline info input.pdf --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --json
```

## Bauen und starten

Installiere Rust 1.95+ und die nativen Bibliotheken deiner Plattform. Der erste Download der Abhängigkeiten braucht eine Verbindung. Für Linux stehen die Systempakete in der [nativen CI](../../.github/workflows/rust.yml).

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

Unter Windows starte `target\release\pdf-editor-offline-native.exe`. Seiten werden ab 1 gezählt. `--dry-run` prüft ohne zu schreiben. Wähle einen neuen Ausgabepfad: bestehende Dateien und symbolische Links werden nie überschrieben.

## Lokal und nachvollziehbar

PDF-Operationen laufen auf deinem Rechner, ohne entfernten Rendering- oder Konvertierungsdienst. Das beweist weder die Sicherheit jedes PDFs noch das Entfernen aller versteckten Informationen. Der [CLI-Nachweis](../RUST_BENCHMARKS.md) enthält 44 Operationsprüfungen mit qpdf/Poppler, Prozesszeiten und Hashes, einschließlich eines synthetischen PDFs mit 1.000 Seiten. Daraus folgen keine allgemeine Dokumenttreue, Desktop-Reaktionszeit oder universelle Beschleunigung.

Hilfreich sind reproduzierbare Fehler mit synthetischen PDFs, Rendering-Vergleiche, Barrierefreiheitstests, Übersetzungskorrekturen und kleine Rust-Änderungen. Teile keine privaten Dokumente in öffentlichen Berichten.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

Wenn dir ein lokaler PDF-Editor wichtig ist, gib dem Repository einen Stern, damit andere ihn leichter finden.

MIT-Lizenz; Schriften und Abhängigkeiten behalten ihre eigenen Hinweise.
