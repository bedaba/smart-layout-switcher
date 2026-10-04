import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

type Settings = {
  enabled: boolean;
  switchLayout: boolean;
  launchAtLogin: boolean;
  confidence: number;
  excludedApps: string[];
};

type RuntimeStatus = {
  platform: string;
  supported: boolean;
  permission: "ready" | "needs_permission" | "unsupported";
  message: string;
  layout: string | null;
};

const defaults: Settings = {
  enabled: true,
  switchLayout: true,
  launchAtLogin: false,
  confidence: 0.8,
  excludedApps: [],
};

function App() {
  const [settings, setSettings] = useState<Settings>(defaults);
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const [newApp, setNewApp] = useState("");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    const saved = localStorage.getItem("badelha-settings-v1");
    if (saved) {
      try {
        const parsed = { ...defaults, ...JSON.parse(saved) } as Settings;
        setSettings(parsed);
        void invoke("update_settings", { settings: parsed });
      } catch {
        localStorage.removeItem("badelha-settings-v1");
      }
    }
    void refreshStatus();
    const statusRetry = window.setTimeout(() => void refreshStatus(), 500);
    const correctionListener = listen("badelha-correction", (event) => {
      const correction = event.payload as { language: string };
      setNotice(correction.language === "ar" ? "تم تصحيح الكلمة إلى العربية." : "تم تصحيح الكلمة إلى الإنجليزية.");
    });
    const enabledListener = listen("badelha-enabled", (event) => {
      const enabled = Boolean(event.payload);
      setSettings((current) => {
        const next = { ...current, enabled };
        localStorage.setItem("badelha-settings-v1", JSON.stringify(next));
        void invoke("update_settings", { settings: next });
        return next;
      });
    });
    return () => {
      window.clearTimeout(statusRetry);
      void correctionListener.then((stop) => stop());
      void enabledListener.then((stop) => stop());
    };
  }, []);

  async function refreshStatus() {
    try {
      setStatus(await invoke<RuntimeStatus>("get_runtime_status"));
    } catch {
      setStatus({
        platform: "unknown",
        supported: false,
        permission: "unsupported",
        message: "تعذر الاتصال بمحرك الكتابة.",
        layout: null,
      });
    }
  }

  async function save(next: Settings) {
    setSettings(next);
    localStorage.setItem("badelha-settings-v1", JSON.stringify(next));
    try {
      await invoke("update_settings", { settings: next });
    } catch {
      setNotice("لم أتمكن من حفظ الإعدادات.");
    }
  }

  async function toggleAutostart(checked: boolean) {
    try {
      if (checked) await enable();
      else await disable();
      const actual = await isEnabled();
      await save({ ...settings, launchAtLogin: actual });
      setNotice(actual ? "سيبدأ بدّلها مع تشغيل الجهاز." : "تم إيقاف التشغيل التلقائي.");
    } catch {
      setNotice("تعذر تغيير إعداد التشغيل التلقائي على هذا النظام.");
    }
  }

  function addExcludedApp() {
    const app = newApp.trim().replace(/\\/g, "/").split("/").pop()?.toLowerCase();
    if (!app || settings.excludedApps.includes(app)) return;
    void save({ ...settings, excludedApps: [...settings.excludedApps, app] });
    setNewApp("");
  }

  const enabled = settings.enabled && (status?.supported ?? false);

  return (
    <main className="shell">
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true"><span>ع</span><span>A</span></div>
          <div><p className="eyebrow">مساعد الكتابة المحلي</p><h1>بدّلها</h1></div>
        </div>
        <div className={`status-pill ${enabled ? "is-on" : "is-off"}`}>
          <span className="status-dot" />{enabled ? "الحماية شغّالة" : "متوقف مؤقتًا"}
        </div>
      </header>

      <section className="hero-card">
        <div className="hero-copy">
          <div className="eyebrow accent">تصحيح تلقائي • عربي ↔ English</div>
          <h2>اكتب براحتك.<br /><span>بدّلها تلحقك.</span></h2>
          <p>لو كتبت كلمة بتخطيط غلط وضغطت مسافة، بدّلها يحاول يصلحها ويحوّل تخطيط لوحة المفاتيح للغة الصحيحة.</p>
          <button className={`primary-button ${settings.enabled ? "pause" : "resume"}`} disabled={!status?.supported} onClick={() => void save({ ...settings, enabled: !settings.enabled })}>
            <span>{status?.supported ? settings.enabled ? "Ⅱ" : "▶" : "!"}</span>{status?.supported ? settings.enabled ? "إيقاف مؤقت" : "تشغيل التصحيح" : "غير متاح على هذا النظام"}
          </button>
        </div>
        <div className="keyboard-art" aria-hidden="true">
          <div className="orbit orbit-one" /><div className="orbit orbit-two" />
          <div className="key key-ar">عربي</div><div className="swap">⇄</div><div className="key key-en">EN</div>
          <div className="typing-line"><i /><i /><i /><i /><i /><i /><i /></div>
        </div>
      </section>

      <section className="status-banner" aria-live="polite">
        <div className="status-icon">{status?.supported ? "✓" : "!"}</div>
        <div className="status-text"><strong>{status?.supported ? "جاهز لمراقبة الكتابة" : "حالة دعم النظام"}</strong><span>{status?.message ?? "جارٍ فحص دعم هذا الجهاز…"}</span></div>
        <span className="platform-tag">{status?.platform ?? "…"}</span>
      </section>

      <div className="section-heading"><div><p className="eyebrow">تحكم في تجربتك</p><h2>الإعدادات</h2></div><span className="local-badge"><span /> كل شيء على جهازك</span></div>

      <section className="settings-grid">
        <article className="setting-card">
          <div className="setting-icon blue">⌨</div><div className="setting-content"><h3>تبديل تخطيط لوحة المفاتيح</h3><p>حوّل التخطيط تلقائيًا للغة الكلمة التي تم تصحيحها.</p></div>
          <label className="switch" aria-label="تبديل تخطيط لوحة المفاتيح"><input type="checkbox" checked={settings.switchLayout} onChange={(e) => void save({ ...settings, switchLayout: e.target.checked })} /><span /></label>
        </article>
        <article className="setting-card">
          <div className="setting-icon violet">◷</div><div className="setting-content"><h3>التشغيل مع الجهاز</h3><p>افتح بدّلها تلقائيًا عند تسجيل الدخول.</p></div>
          <label className="switch" aria-label="التشغيل مع الجهاز"><input type="checkbox" checked={settings.launchAtLogin} onChange={(e) => void toggleAutostart(e.target.checked)} /><span /></label>
        </article>
      </section>

      <section className="lower-grid">
        <article className="panel confidence-panel">
          <div className="panel-heading"><div><h3>دقة التصحيح</h3><p>لو الكلمة مش واضحة، بدّلها هيسيبها زي ما هي.</p></div><span className="confidence-value">{Math.round(settings.confidence * 100)}%</span></div>
          <input className="range" type="range" min="60" max="100" step="5" value={Math.round(settings.confidence * 100)} onChange={(e) => void save({ ...settings, confidence: Number(e.target.value) / 100 })} aria-label="مستوى دقة التصحيح" />
          <div className="range-labels"><span>تصحيح أكثر</span><span>تحفظ أكبر</span></div>
          <div className="confidence-note"><span>✦</span> لن تُرسل الكلمات إلى الإنترنت، ولن يحتفظ التطبيق بسجل لها.</div>
        </article>
        <article className="panel exclusions-panel">
          <div className="panel-heading"><div><h3>تجاهل تطبيقات معينة</h3><p>أضف اسم ملف التطبيق التنفيذي، مثل <bdi>game.exe</bdi>.</p></div><span className="small-count">{settings.excludedApps.length}</span></div>
          <div className="add-app"><input value={newApp} onChange={(e) => setNewApp(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addExcludedApp()} placeholder="اسم التطبيق، مثل game.exe" aria-label="اسم التطبيق المستثنى" /><button onClick={addExcludedApp} disabled={!newApp.trim()}>إضافة</button></div>
          {settings.excludedApps.length > 0 ? <ul className="app-list">{settings.excludedApps.map((app) => <li key={app}><span className="app-dot" />{app}<button aria-label={`إزالة ${app}`} onClick={() => void save({ ...settings, excludedApps: settings.excludedApps.filter((item) => item !== app) })}>×</button></li>)}</ul> : <div className="empty-list">لا توجد تطبيقات مستثناة</div>}
        </article>
      </section>

      {notice && <div className="toast" role="status">{notice}<button onClick={() => setNotice("")}>×</button></div>}
      <footer><span>بدّلها <b>0.1.0</b></span><span>مصمم ليحافظ على خصوصيتك <i>●</i></span></footer>
    </main>
  );
}

export default App;
