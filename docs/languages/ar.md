# PDF Editor Offline

**محرّر PDF يعمل دون اتصال، مبني بلغة Rust. تبقى ملفاتك على جهازك.**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

افتح ملف PDF، وابحث عن الصفحة المطلوبة، ورتّب المستند ثم احفظ نسخة جديدة. PDF Editor Offline تطبيق مكتبي وأداة سطر أوامر مجانيان ومفتوحا المصدر. لا يتطلب حسابًا أو اشتراكًا أو رفع المستندات.

هذه نسخة معاينة لتطبيق Rust الأصلي. اختبر تطبيق سطح المكتب الحالي ببنائه من المصدر. المنصات المستهدفة هي macOS وWindows وLinux؛ أغلب التحقق الفعلي من واجهة سطح المكتب تم على macOS. حزم تثبيت سطح المكتب والتحرير الكامل ما زالت قيد التطوير. [نسخة CLI الأصلية التجريبية 4.0.0-preview.1](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.1) متاحة؛ [إصدارات سطح المكتب المؤرشفة](https://github.com/OthmaneBlial/pdf-editor-offline/releases) تخص نسخًا سابقة.

![نافذة حقيقية للتطبيق على macOS تعرض البحث المحلي وصفحة PDF ومعلومات المستند](../../screenshots/native/editorial-light-2026-10-12.png)

## الوظائف المتاحة

| المهمة | المتاح |
| --- | --- |
| القراءة والبحث | علامات تبويب، تكبير، صور مصغّرة للصفحات الظاهرة، نص، بحث حرفي ومظهر فاتح أو داكن؛ معلومات واستخراج نص ومقارنة محدودة في سطر الأوامر. |
| ترتيب الصفحات | نقل الصفحات وعكس ترتيبها وتدويرها وإضافة صفحات فارغة وقص العرض والتراجع والإعادة في التطبيق؛ استخراج وحذف محميان ودمج ملفات ذات كتالوجات بسيطة في سطر الأوامر. |
| خصائص المستند | قراءة وتعديل ستة حقول قياسية تدعم Unicode: Title وAuthor وSubject وKeywords وCreator وProducer. |
| الصور والتعليقات | إنشاء صفحات PDF من صور PNG/JPEG المدعومة؛ تصدير صفحة إلى PNG وإضافة ملاحظات Text وعرضها وتعديلها أو حذفها ضمن قيود في سطر الأوامر. |
| الفحص والحفظ | عرض مؤشرات الإجراءات والمرفقات والنماذج والطبقات للقراءة فقط في سطر الأوامر؛ ضغط تدفقات دون فقد وحفظ نسخ جديدة متحقّق منها دون استبدال الملفات. |

للدمج ومجموعات الصفحات والملاحظات وتحويل الصور قيود واضحة. تُرفض البنى غير المدعومة؛ راجع [دليل سطر الأوامر](../../apps/cli/README.md). الضغط لا يضمن حجمًا أصغر. قص العرض يخفي المحتوى؛ إزالة البيانات الوصفية أو الملاحظات قد تُبقي قيمًا قديمة. هذه العمليات ليست محوًا آمنًا للمعلومات. التعرّف الضوئي OCR وتعبئة النماذج والتوقيع الرقمي وتحرير نص الصفحة بحرية والتحويل العام غير متاحة. قد لا يعرض محرّك الرسم محتوى غير مدعوم، لذا راجع الملفات المُصدّرة. المستندات الموقعة أو المفكوك تشفيرها بنجاح تبقى للقراءة فقط.

واجهة التطبيق حاليًا باللغة الإنجليزية. ترجمة هذا الدليل لا تعني تعريب الواجهة أو دعمًا كاملًا للتخطيط من اليمين إلى اليسار. الخطوط المحلية تتضمن محارف صينية ويابانية وكورية تم اختبارها، لكنها لا تغطي جميع أنظمة الكتابة.

## تثبيت CLI الأصلية (اختياري)

تحتاج عملية التثبيت عبر npm وبرنامج التشغيل إلى Node.js 22 أو أحدث. ملف CLI التنفيذي المستقل لا يحتاج إلى Node.js. تطبيق سطح المكتب ما زال يتطلب البناء من مصدر Rust.

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

### جرّب ملف PDF الخاص بك

استبدل `input.pdf` بملف محلي. يقرأ الأمر الأول معلوماته، ويتحقق الثاني من التدوير دون كتابة، وينشئ الثالث نسخة جديدة. اختر اسمًا لملف الإخراج غير موجود مسبقًا.

```sh
pdf-editor-offline info input.pdf --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --json
```

## البناء والتشغيل

ثبّت Rust 1.95 أو أحدث ومكتبات البناء الأصلية لمنصتك. تنزيل الاعتماديات لأول مرة يحتاج اتصالًا. على Linux، راجع [حزم البناء في CI](../../.github/workflows/rust.yml).

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

على Windows شغّل `target\release\pdf-editor-offline-native.exe`. يبدأ ترقيم الصفحات من 1. يتحقق `--dry-run` دون كتابة ملفات. اختر مسار إخراج جديدًا؛ لا تُستبدل الملفات أو الروابط الرمزية الموجودة.

## معالجة محلية ونتائج قابلة للفحص

تُجرى العمليات على جهازك دون خدمة عرض أو تحويل سحابية. هذا لا يثبت سلامة كل ملف PDF أو إزالة جميع المعلومات المخفية. يسجّل [تقرير CLI](../RUST_BENCHMARKS.md) 44 فحصًا للعمليات باستخدام qpdf/Poppler، وأزمنة العمليات والبصمات، بما فيها ملف تجريبي من 1,000 صفحة. لا تثبت هذه القياسات دقة جميع الملفات أو استجابة الواجهة أو تسريعًا عامًا.

يمكنك المساهمة بملفات تجريبية اصطناعية لإعادة إنتاج المشكلات، ومقارنات الرسم واختبارات إمكانية الوصول وتصحيح الترجمات وتغييرات Rust الصغيرة. لا تُرفق مستندات خاصة في البلاغات العامة.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

إذا كان محرّر PDF محلي مفيدًا لك، امنح المستودع نجمة ليسهل على الآخرين العثور عليه.

مرخّص وفق MIT؛ للخطوط والاعتماديات إشعارات ترخيص خاصة بها.
