<div align="center">

<img src="assets/badelha-banner.svg" alt="بدّلها — Badelha" width="100%" />

# بدّلها · Badelha

**مساعد محلي لتصحيح تخطيط لوحة المفاتيح العربية والإنجليزية**

[العربية](#العربية) · [English](#english)

[![Latest release](https://img.shields.io/github/v/release/bedaba/smart-layout-switcher?style=for-the-badge&color=85a9ff)](https://github.com/bedaba/smart-layout-switcher/releases/latest)
[![Build desktop packages](https://img.shields.io/github/actions/workflow/status/bedaba/smart-layout-switcher/build.yml?branch=main&label=build&style=for-the-badge)](https://github.com/bedaba/smart-layout-switcher/actions/workflows/build.yml)
![Tauri 2](https://img.shields.io/badge/Tauri-2-9299ff?style=for-the-badge)

</div>

## العربية

بدّلها تطبيق سطح مكتب يعمل بجانب الساعة. يراجع الكلمة عند كتابة مسافة أو علامة ترقيم، ويحوّلها بين تخطيطي العربية والإنجليزية عند وجود تطابق واضح. التصحيح محلي ولا يحتاج إلى اتصال بالإنترنت.

### المميزات

- واجهة عربية وإنجليزية مع تبديل دائم واتجاه عرض مناسب.
- محرك محلي يستخدم خريطة التخطيط وقوائم كلمات شائعة وسياقًا قصيرًا وقاموسًا شخصيًا.
- مستوى ثقة قابل للضبط؛ الكلمات الملتبسة تبقى كما كُتبت.
- تراجع سريع عن آخر تصحيح، واختصار لتحويل النص المحدد.
- تشغيل تلقائي، إيقاف مؤقت، واستثناء تطبيقات.
- لا يحتفظ بسجل للكتابة؛ سياق التصحيح مؤقت في الذاكرة.

### الأنظمة والحالة

| النظام | الحالة الحالية |
|---|---|
| Windows | موائم إدخال يستخدم خطاف لوحة المفاتيح ويدعم كشف عناصر كلمات المرور القياسية. |

**نطاق الإصدار الحالي:** Windows فقط. لا تتوفر حزم أو موائمات إدخال مدعومة لـ macOS وLinux حاليًا. فحص الحقول الآمنة واستثناء التطبيقات متاحان في موائم Windows. يعرض التطبيق حالة المحرك عند بدء التشغيل.

### التحميل والتطوير

حمّل آخر إصدار من [صفحة الإصدارات](https://github.com/bedaba/smart-layout-switcher/releases). ينشئ GitHub Actions حزم Windows بصيغتي NSIS وMSI.

للتطوير، ثبّت Node.js وRust ومتطلبات Tauri للنظام ثم شغّل:

```sh
npm ci
npm run tauri dev
```

لإنشاء حزم النظام الحالي:

```sh
npm run tauri build
```

### المساهمة والإبلاغ

يرجى فتح [بلاغ مشكلة](https://github.com/bedaba/smart-layout-switcher/issues) أو [مساهمة](https://github.com/bedaba/smart-layout-switcher/pulls). تتوفر قوالب عربية وإنجليزية. لا تضع كلمات أو بيانات خاصة في البلاغات.

---

## English

Badelha is a desktop typing assistant that lives in the system tray. It checks a word after a space or punctuation mark and converts it between Arabic and English keyboard layouts when the match is clear. Correction runs locally and works offline.

### Features

- Arabic and English interface with a persistent language toggle and matching text direction.
- Local layout mapping, common-word lists, short context, and a personal dictionary.
- Adjustable confidence threshold; ambiguous words are left unchanged.
- Quick undo for the latest correction and a shortcut to convert selected text.
- Startup option, pause control, and application exclusions.
- No typing history is stored; correction context stays temporarily in memory.

### Platforms and status

| Platform | Current status |
|---|---|
| Windows | Input adapter uses a keyboard hook and detects standard password controls. |

**Current scope:** Windows only. Supported input adapters and release packages are not currently available for macOS or Linux. Secure-field detection and application exclusions are available in the Windows adapter. The app reports the engine status at startup.

### Downloads and development

Download the latest release from [GitHub Releases](https://github.com/bedaba/smart-layout-switcher/releases). GitHub Actions builds Windows packages in NSIS and MSI formats.

Install Node.js, Rust, and the Tauri prerequisites for your operating system, then run:

```sh
npm ci
npm run tauri dev
```

Build packages for the current operating system with:

```sh
npm run tauri build
```

### Contributing and reporting

Open an [issue](https://github.com/bedaba/smart-layout-switcher/issues) or a [pull request](https://github.com/bedaba/smart-layout-switcher/pulls). Bilingual templates are available. Please do not include private typing data in reports.
