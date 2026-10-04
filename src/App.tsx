import { useEffect, useState } from "react";
import type { KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";

type Language = "ar" | "en";
type Settings = {
  enabled: boolean;
  switchLayout: boolean;
  launchAtLogin: boolean;
  confidence: number;
  excludedApps: string[];
  customWords: string[];
  interfaceLanguage: Language;
  convertShortcut: string;
  undoShortcut: string;
};
type RuntimeStatus = {
  platform: string;
  supported: boolean;
  permission: "ready" | "needs_permission" | "unsupported";
  message: string;
  messageEn: string;
  layout: string | null;
};
type Correction = { from: string; to: string; language: string; confidence: number };

const baseDefaults = {
  enabled: true,
  switchLayout: true,
  launchAtLogin: false,
  confidence: 0.8,
  excludedApps: [] as string[],
  customWords: [] as string[],
  convertShortcut: "CommandOrControl+Shift+J",
  undoShortcut: "CommandOrControl+Shift+Z",
};

const words = {
  ar: {
    tagline: "مساعد الكتابة المحلي", active: "الحماية شغّالة", paused: "متوقف مؤقتًا",
    heroTag: "تصحيح تلقائي · عربي ↔ English", headline1: "اكتب براحتك.", headline2: "بدّلها تلحقك.",
    intro: "يكتشف بدّلها الكلمات المكتوبة بتخطيط غير صحيح، ويصلحها محليًا عند وجود تصحيح واضح.",
    pause: "إيقاف مؤقت", resume: "تشغيل التصحيح", unavailable: "غير متاح على هذا النظام",
    statusReady: "جاهز لمراقبة الكتابة", statusChecking: "حالة دعم النظام", statusUnsupported: "الدعم غير متاح حاليًا", permissionRequired: "الصلاحية مطلوبة", checking: "جارٍ فحص دعم هذا الجهاز…",
    controls: "تحكم في تجربتك", settings: "الإعدادات", local: "كل شيء على جهازك",
    switchTitle: "تبديل تخطيط لوحة المفاتيح", switchDesc: "حوّل التخطيط تلقائيًا للغة الكلمة التي تم تصحيحها.",
    startupTitle: "التشغيل مع الجهاز", startupDesc: "افتح بدّلها تلقائيًا عند تسجيل الدخول.",
    accuracy: "دقة التصحيح", accuracyDesc: "عند الشك، سيترك بدّلها الكلمة دون تعديل.",
    moreCorrections: "تصحيح أكثر", safer: "تحفظ أكبر", localNote: "لا تُرسل الكلمات إلى الإنترنت ولا يُحفَظ سجل لها.",
    dictionary: "قاموسك الشخصي", dictionaryDesc: "أضف أسماءك ومصطلحاتك كي لا يغيّرها التصحيح التلقائي.",
    addWordPlaceholder: "اكتب كلمة لإضافتها", add: "إضافة", remove: "إزالة", noWords: "القاموس فارغ حاليًا",
    exclusions: "تجاهل تطبيقات معينة", exclusionsDesc: "أضف اسم ملف التطبيق التنفيذي، مثل game.exe.", exclusionsWindowsOnly: "قائمة الاستثناءات وفحص حقول كلمات المرور متاحان حاليًا في Windows فقط.",
    appPlaceholder: "اسم التطبيق، مثل game.exe", noApps: "لا توجد تطبيقات مستثناة",
    shortcuts: "الاختصارات", shortcutsDesc: "اختصار النص المحدد يستخدم الحافظة مؤقتًا ثم يعيد نصها السابق.",
    convertSelected: "تحويل النص المحدد", undoShortcut: "تراجع عن آخر تصحيح", shortcutHint: "انقر على الحقل ثم اضغط مجموعة المفاتيح المطلوبة.",
    toastCorrected: "تم تصحيح الكلمة", undoHint: "للتراجع فورًا اضغط", undoDone: "تم التراجع عن التصحيح.",
    noSelection: "لم يتم العثور على نص محدد لتحويله.", selectionUnclear: "النص المحدد مختلط أو لا يمكن تحويله بأمان.",
    clipboardUnavailable: "تعذر حفظ نص الحافظة لاستعادته؛ لم يتم تغييرها.", correctedSelection: "تم تحويل النص المحدد.",
    saveFailed: "لم أتمكن من حفظ الإعدادات.", autostartOn: "سيبدأ بدّلها مع تشغيل الجهاز.", autostartOff: "تم إيقاف التشغيل التلقائي.",
    autostartFailed: "تعذر تغيير إعداد التشغيل التلقائي على هذا النظام.", shortcutsFailed: "الاختصار غير متاح أو مستخدم من تطبيق آخر.",
    platformWindows: "Windows x64", footerPrivacy: "مصمم ليحافظ على خصوصيتك", language: "اللغة",
  },
  en: {
    tagline: "Your local typing assistant", active: "Protection is on", paused: "Paused",
    heroTag: "Automatic correction · عربي ↔ English", headline1: "Type freely.", headline2: "Badelha catches up.",
    intro: "Badelha detects words typed with the wrong keyboard layout and fixes them locally when the match is clear.",
    pause: "Pause correction", resume: "Enable correction", unavailable: "Unavailable on this system",
    statusReady: "Ready to monitor typing", statusChecking: "System support", statusUnsupported: "Support is currently unavailable", permissionRequired: "Permission required", checking: "Checking this device…",
    controls: "Make it yours", settings: "Settings", local: "Everything stays on your device",
    switchTitle: "Switch keyboard layout", switchDesc: "Switch to the language of a corrected word automatically.",
    startupTitle: "Start with your computer", startupDesc: "Open Badelha automatically when you sign in.",
    accuracy: "Correction confidence", accuracyDesc: "When uncertain, Badelha leaves the word unchanged.",
    moreCorrections: "More corrections", safer: "More cautious", localNote: "Words are never sent online or saved as a history.",
    dictionary: "Personal dictionary", dictionaryDesc: "Add names and terms that automatic correction should keep.",
    addWordPlaceholder: "Add a word", add: "Add", remove: "Remove", noWords: "Your dictionary is empty",
    exclusions: "Excluded applications", exclusionsDesc: "Add an executable name, such as game.exe.", exclusionsWindowsOnly: "Application exclusions and password-field detection are currently available on Windows only.",
    appPlaceholder: "Application name, e.g. game.exe", noApps: "No excluded applications",
    shortcuts: "Shortcuts", shortcutsDesc: "Selected-text conversion temporarily uses the clipboard, then restores its previous text.",
    convertSelected: "Convert selected text", undoShortcut: "Undo last correction", shortcutHint: "Focus the field, then press the shortcut you want.",
    toastCorrected: "Word corrected", undoHint: "Undo right away with", undoDone: "Correction undone.",
    noSelection: "No selected text was found.", selectionUnclear: "The selection is mixed or cannot be converted safely.",
    clipboardUnavailable: "Could not save the clipboard text for restoration; it was left unchanged.", correctedSelection: "Selected text converted.",
    saveFailed: "Could not save settings.", autostartOn: "Badelha will start when your computer starts.", autostartOff: "Automatic startup is off.",
    autostartFailed: "Could not change the startup setting on this system.", shortcutsFailed: "That shortcut is unavailable or already in use.",
    platformWindows: "Windows x64", footerPrivacy: "Designed with your privacy in mind", language: "Language",
  },
} as const;

function initialSettings(): Settings {
  const language: Language = navigator.language.toLowerCase().startsWith("ar") ? "ar" : "en";
  const saved = localStorage.getItem("badelha-settings-v2") ?? localStorage.getItem("badelha-settings-v1");
  if (saved) {
    try {
      const previous = JSON.parse(saved);
      const value = { ...baseDefaults, ...previous };
      value.interfaceLanguage = previous.interfaceLanguage === "en" || previous.interfaceLanguage === "ar" ? previous.interfaceLanguage : language;
      return value as Settings;
    } catch {
      localStorage.removeItem("badelha-settings-v1");
      localStorage.removeItem("badelha-settings-v2");
    }
  }
  return { ...baseDefaults, interfaceLanguage: language };
}

function App() {
  const [settings, setSettings] = useState<Settings>(initialSettings);
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const [newApp, setNewApp] = useState("");
  const [newWord, setNewWord] = useState("");
  const [notice, setNotice] = useState("");
  const [recordingShortcut, setRecordingShortcut] = useState(false);
  const t = words[settings.interfaceLanguage];
  const isArabic = settings.interfaceLanguage === "ar";

  useEffect(() => {
    document.documentElement.lang = settings.interfaceLanguage;
    document.documentElement.dir = isArabic ? "rtl" : "ltr";
    document.body.lang = settings.interfaceLanguage;
    document.body.dir = isArabic ? "rtl" : "ltr";
  }, [settings.interfaceLanguage, isArabic]);

  useEffect(() => {
    localStorage.setItem("badelha-settings-v2", JSON.stringify(settings));
    void invoke("update_settings", { settings });
  }, [settings]);

  useEffect(() => {
    void refreshStatus();
    const statusRetry = window.setTimeout(() => void refreshStatus(), 500);
    const correctionListener = listen("badelha-correction", (event) => {
      const correction = event.payload as Correction;
      setNotice(`${t.toastCorrected}: ${correction.from} → ${correction.to}. ${t.undoHint} ${settings.undoShortcut}`);
    });
    const undoListener = listen("badelha-undo", () => setNotice(t.undoDone));
    const enabledListener = listen("badelha-enabled", (event) => {
      setSettings((current) => ({ ...current, enabled: Boolean(event.payload) }));
    });
    return () => {
      window.clearTimeout(statusRetry);
      void correctionListener.then((stop) => stop());
      void undoListener.then((stop) => stop());
      void enabledListener.then((stop) => stop());
    };
  // Settings are read for the current localized notice only.
  }, [t, settings.undoShortcut]);

  const refreshStatus = async () => {
    try {
      setStatus(await invoke<RuntimeStatus>("get_runtime_status"));
    } catch {
      setStatus({ platform: "unknown", supported: false, permission: "unsupported", message: "تعذر الاتصال بمحرك الكتابة.", messageEn: "Could not connect to the typing engine.", layout: null });
    }
  };

  async function save(next: Settings) {
    setSettings(next);
    try {
      await invoke("update_settings", { settings: next });
    } catch {
      setNotice(t.saveFailed);
    }
  }

  useEffect(() => {
    if (recordingShortcut) return;
    let active = true;
    const shortcuts = [settings.convertShortcut, settings.undoShortcut];
    void (async () => {
      try {
        await register(settings.convertShortcut, (event) => {
          if (event.state === "Pressed") void convertSelection();
        });
        await register(settings.undoShortcut, (event) => {
          if (event.state === "Pressed") void invoke("undo_last_correction");
        });
      } catch {
        if (active) setNotice(t.shortcutsFailed);
      }
    })();
    return () => {
      active = false;
      for (const shortcut of shortcuts) void unregister(shortcut).catch(() => undefined);
    };
  }, [settings.convertShortcut, settings.undoShortcut, recordingShortcut, t]);

  function captureShortcut(event: KeyboardEvent<HTMLInputElement>) {
    event.preventDefault();
    if (event.key === "Escape") {
      setRecordingShortcut(false);
      event.currentTarget.blur();
      return;
    }
    if (["Control", "Shift", "Alt", "Meta"].includes(event.key)) return;
    const parts: string[] = [];
    if (event.ctrlKey || event.metaKey) parts.push("CommandOrControl");
    if (event.altKey) parts.push("Alt");
    if (event.shiftKey) parts.push("Shift");
    const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
    if (!parts.length || !key) return;
    parts.push(key);
    void save({ ...settings, convertShortcut: parts.join("+") });
    setRecordingShortcut(false);
    event.currentTarget.blur();
  }

  async function convertSelection() {
    let previousClipboard: string;
    try {
      previousClipboard = await readText();
    } catch {
      setNotice(t.clipboardUnavailable);
      return;
    }
    try {
      await writeText("");
      await invoke("send_copy_shortcut");
      await new Promise((resolve) => window.setTimeout(resolve, 120));
      const selected = await readText();
      if (!selected.trim()) {
        setNotice(t.noSelection);
        return;
      }
      const converted = await invoke<string | null>("correct_selection", { text: selected });
      if (!converted) {
        setNotice(t.selectionUnclear);
        return;
      }
      await writeText(converted);
      await invoke("send_paste_shortcut");
      await new Promise((resolve) => window.setTimeout(resolve, 250));
      setNotice(t.correctedSelection);
    } catch {
      setNotice(t.selectionUnclear);
    } finally {
      try { await writeText(previousClipboard); } catch { /* Clipboard restoration is best effort. */ }
    }
  }

  async function toggleAutostart(checked: boolean) {
    try {
      if (checked) await enable(); else await disable();
      const actual = await isEnabled();
      await save({ ...settings, launchAtLogin: actual });
      setNotice(actual ? t.autostartOn : t.autostartOff);
    } catch {
      setNotice(t.autostartFailed);
    }
  }

  function addExcludedApp() {
    const app = newApp.trim().replace(/\\/g, "/").split("/").pop()?.toLowerCase();
    if (!app || settings.excludedApps.includes(app)) return;
    void save({ ...settings, excludedApps: [...settings.excludedApps, app] });
    setNewApp("");
  }

  function addCustomWord() {
    const word = newWord.trim();
    if (!word || settings.customWords.some((known) => known.toLocaleLowerCase() === word.toLocaleLowerCase())) return;
    void save({ ...settings, customWords: [...settings.customWords, word] });
    setNewWord("");
  }

  const isOn = settings.enabled && (status?.supported ?? false);
  const statusMessage = isArabic ? status?.message : status?.messageEn;
  const statusHeadline = status?.supported
    ? t.statusReady
    : status?.permission === "needs_permission"
      ? t.permissionRequired
      : status?.permission === "unsupported"
        ? t.statusUnsupported
        : t.statusChecking;
  const supportsAppExclusions = status?.platform === "Windows";

  return (
    <main className="shell" dir={isArabic ? "rtl" : "ltr"}>
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true"><span>ع</span><span>A</span></div>
          <div><p className="eyebrow">{t.tagline}</p><h1>بدّلها <span>Badelha</span></h1></div>
        </div>
        <div className="top-actions">
          <div className="language-toggle" aria-label={t.language}>
            <button className={isArabic ? "selected" : ""} onClick={() => void save({ ...settings, interfaceLanguage: "ar" })}>عربي</button>
            <button className={!isArabic ? "selected" : ""} onClick={() => void save({ ...settings, interfaceLanguage: "en" })}>English</button>
          </div>
          <div className={`status-pill ${isOn ? "is-on" : "is-off"}`}><span className="status-dot" />{isOn ? t.active : t.paused}</div>
        </div>
      </header>

      <section className="hero-card">
        <div className="hero-copy">
          <div className="eyebrow accent">{t.heroTag}</div>
          <h2>{t.headline1}<br /><span>{t.headline2}</span></h2>
          <p>{t.intro}</p>
          <button className={`primary-button ${settings.enabled ? "pause" : "resume"}`} disabled={!status?.supported} onClick={() => void save({ ...settings, enabled: !settings.enabled })}>
            <span>{status?.supported ? settings.enabled ? "Ⅱ" : "▶" : "!"}</span>{status?.supported ? settings.enabled ? t.pause : t.resume : t.unavailable}
          </button>
        </div>
        <div className="keyboard-art" aria-hidden="true"><div className="orbit orbit-one" /><div className="orbit orbit-two" /><div className="key key-ar">عربي</div><div className="swap">⇄</div><div className="key key-en">EN</div><div className="typing-line"><i /><i /><i /><i /><i /><i /><i /></div></div>
      </section>

      <section className="status-banner" aria-live="polite">
        <div className="status-icon">{status?.supported ? "✓" : "!"}</div>
        <div className="status-text"><strong>{statusHeadline}</strong><span>{statusMessage ?? t.checking}</span></div>
        <span className="platform-tag">{status?.platform ?? "…"}</span>
      </section>

      <div className="section-heading"><div><p className="eyebrow">{t.controls}</p><h2>{t.settings}</h2></div><span className="local-badge"><span />{t.local}</span></div>

      <section className="settings-grid">
        <article className="setting-card"><div className="setting-icon blue">⌨</div><div className="setting-content"><h3>{t.switchTitle}</h3><p>{t.switchDesc}</p></div><label className="switch" aria-label={t.switchTitle}><input type="checkbox" checked={settings.switchLayout} onChange={(e) => void save({ ...settings, switchLayout: e.target.checked })} /><span /></label></article>
        <article className="setting-card"><div className="setting-icon violet">◷</div><div className="setting-content"><h3>{t.startupTitle}</h3><p>{t.startupDesc}</p></div><label className="switch" aria-label={t.startupTitle}><input type="checkbox" checked={settings.launchAtLogin} onChange={(e) => void toggleAutostart(e.target.checked)} /><span /></label></article>
      </section>

      <section className="lower-grid">
        <article className="panel confidence-panel">
          <div className="panel-heading"><div><h3>{t.accuracy}</h3><p>{t.accuracyDesc}</p></div><span className="confidence-value">{Math.round(settings.confidence * 100)}%</span></div>
          <input className="range" type="range" min="60" max="100" step="5" value={Math.round(settings.confidence * 100)} onChange={(e) => void save({ ...settings, confidence: Number(e.target.value) / 100 })} aria-label={t.accuracy} />
          <div className="range-labels"><span>{t.moreCorrections}</span><span>{t.safer}</span></div><div className="confidence-note"><span>✦</span>{t.localNote}</div>
        </article>
        <article className="panel dictionary-panel">
          <div className="panel-heading"><div><h3>{t.dictionary}</h3><p>{t.dictionaryDesc}</p></div><span className="small-count">{settings.customWords.length}</span></div>
          <div className="add-app"><input value={newWord} onChange={(e) => setNewWord(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addCustomWord()} placeholder={t.addWordPlaceholder} aria-label={t.addWordPlaceholder} /><button onClick={addCustomWord} disabled={!newWord.trim()}>{t.add}</button></div>
          {settings.customWords.length > 0 ? <ul className="app-list word-list">{settings.customWords.map((word) => <li key={word}><span className="app-dot" />{word}<button aria-label={`${t.remove} ${word}`} onClick={() => void save({ ...settings, customWords: settings.customWords.filter((item) => item !== word) })}>×</button></li>)}</ul> : <div className="empty-list">{t.noWords}</div>}
        </article>
      </section>

      <section className="lower-grid second-row">
        <article className="panel exclusions-panel">
          <div className="panel-heading"><div><h3>{t.exclusions}</h3><p>{supportsAppExclusions ? t.exclusionsDesc : t.exclusionsWindowsOnly}</p></div><span className="small-count">{settings.excludedApps.length}</span></div>
          <div className="add-app"><input value={newApp} onChange={(e) => setNewApp(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addExcludedApp()} placeholder={t.appPlaceholder} aria-label={t.appPlaceholder} disabled={!supportsAppExclusions} /><button onClick={addExcludedApp} disabled={!supportsAppExclusions || !newApp.trim()}>{t.add}</button></div>
          {settings.excludedApps.length > 0 ? <ul className="app-list">{settings.excludedApps.map((app) => <li key={app}><span className="app-dot" />{app}<button aria-label={`${t.remove} ${app}`} onClick={() => void save({ ...settings, excludedApps: settings.excludedApps.filter((item) => item !== app) })}>×</button></li>)}</ul> : <div className="empty-list">{t.noApps}</div>}
        </article>
        <article className="panel shortcuts-panel">
          <div className="panel-heading"><div><h3>{t.shortcuts}</h3><p>{t.shortcutsDesc}</p></div><span className="setting-icon blue">⌘</span></div>
          <label className="shortcut-row"><span>{t.convertSelected}</span><input value={recordingShortcut ? (isArabic ? "اضغط الاختصار…" : "Press shortcut…") : settings.convertShortcut} onFocus={() => { void unregister(settings.convertShortcut).catch(() => undefined); setRecordingShortcut(true); }} onBlur={() => setRecordingShortcut(false)} onKeyDown={captureShortcut} aria-label={t.convertSelected} /></label>
          <label className="shortcut-row"><span>{t.undoShortcut}</span><input value={settings.undoShortcut} readOnly aria-label={t.undoShortcut} /></label>
          <div className="empty-list">{t.shortcutHint}</div>
        </article>
      </section>

      {notice && <div className="toast" role="status">{notice}<button onClick={() => setNotice("")}>×</button></div>}
      <footer><span>بدّلها <b>0.2.0</b></span><span>{t.footerPrivacy} <i>●</i></span></footer>
    </main>
  );
}

export default App;
