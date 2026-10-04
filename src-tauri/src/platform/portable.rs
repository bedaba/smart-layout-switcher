use super::{correct_token, PlatformInputAdapter};
use crate::{AppSettings, RuntimeStatus};
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use inputs::{
    Event, EventDisposition, EventKind, KeyEvent, Listener, LogicalKey, NamedKey, PressState,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

struct UndoRecord {
    original: String,
    corrected: String,
    delimiter: String,
    created: Instant,
}

struct EngineState {
    settings: AppSettings,
    word: String,
    context: Vec<String>,
    undo: Option<UndoRecord>,
    layout: Option<String>,
    app: AppHandle,
}

static ENGINE: OnceLock<Arc<Mutex<EngineState>>> = OnceLock::new();
static LISTENER: OnceLock<Mutex<Option<Listener>>> = OnceLock::new();
static READY: AtomicBool = AtomicBool::new(false);
static CURRENT_LAYOUT: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub struct PortableInputAdapter;

impl PlatformInputAdapter for PortableInputAdapter {
    fn start(&self, settings: Arc<Mutex<AppSettings>>, app: AppHandle) {
        let initial = settings.lock().map(|s| s.clone()).unwrap_or_default();
        let _ = ENGINE.set(Arc::new(Mutex::new(EngineState {
            settings: initial,
            word: String::new(),
            context: Vec::new(),
            undo: None,
            layout: None,
            app,
        })));

        #[cfg(target_os = "macos")]
        if !inputs::permission_status().is_usable() {
            inputs::request_permission();
        }

        let started = Listener::builder().start(process_event);
        match started {
            Ok(listener) => {
                let _ = LISTENER
                    .get_or_init(|| Mutex::new(None))
                    .lock()
                    .map(|mut slot| *slot = Some(listener));
                READY.store(true, Ordering::Release);
            }
            Err(_) => READY.store(false, Ordering::Release),
        }
    }

    fn status(&self) -> RuntimeStatus {
        let layout = CURRENT_LAYOUT
            .get_or_init(|| Mutex::new(None))
            .lock()
            .ok()
            .and_then(|v| v.clone());
        let ready = READY.load(Ordering::Acquire);
        let (platform, message, message_en) = if cfg!(target_os = "macos") {
            (
                "macOS",
                if ready {
                    "مراقبة الإدخال جاهزة. فحص حقول كلمات المرور واستثناء التطبيقات غير متاحين بعد على macOS؛ أوقف التصحيح في التطبيقات الحساسة."
                } else {
                    "اسمح لـ بدّلها بمراقبة الإدخال وإرسال ضغطات المفاتيح من إعدادات الخصوصية ثم أعد تشغيله."
                },
                if ready {
                    "Input monitoring is ready. Password-field detection and app exclusions are not implemented on macOS yet; pause correction in sensitive apps."
                } else {
                    "Allow Badelha to monitor input and send keystrokes in Privacy settings, then restart it."
                },
            )
        } else {
            (
                "Linux",
                if ready {
                    "مراقبة الإدخال جاهزة. فحص حقول كلمات المرور واستثناء التطبيقات غير متاحين بعد على Linux؛ أوقف التصحيح في التطبيقات الحساسة. دعم Wayland يعتمد على واجهة الإدخال المتاحة."
                } else {
                    "تعذر فتح أجهزة الإدخال. تحقق من صلاحية /dev/input و/dev/uinput ثم أعد تشغيل التطبيق."
                },
                if ready {
                    "Input monitoring is ready. Password-field detection and app exclusions are not implemented on Linux yet; pause correction in sensitive apps. Wayland support depends on the available input backend."
                } else {
                    "Could not open input devices. Check access to /dev/input and /dev/uinput, then restart the app."
                },
            )
        };
        RuntimeStatus {
            platform: platform.into(),
            supported: ready,
            permission: if ready { "ready" } else { "needs_permission" }.into(),
            message: message.into(),
            message_en: message_en.into(),
            layout,
        }
    }

    fn undo_last_correction(&self) -> bool {
        undo_correction()
    }
}

pub fn set_settings(settings: AppSettings) {
    if let Some(engine) = ENGINE.get() {
        if let Ok(mut state) = engine.lock() {
            state.settings = settings;
        }
    }
}

pub fn undo_last_correction() -> bool {
    undo_correction()
}

fn undo_correction() -> bool {
    let Some(engine) = ENGINE.get() else {
        return false;
    };
    let Ok(mut state) = engine.lock() else {
        return false;
    };
    let Some(record) = state.undo.take() else {
        return false;
    };
    if record.created.elapsed() > Duration::from_secs(8) {
        return false;
    }
    let restored = replace_word(&record.corrected, &record.original, &record.delimiter);
    if restored {
        state.word.clear();
        state.context.clear();
        let _ = state
            .app
            .emit("badelha-undo", serde_json::json!({ "restored": true }));
    }
    restored
}

fn process_event(event: &Event) -> EventDisposition {
    if event.injected {
        return EventDisposition::Pass;
    }
    let EventKind::Key(key) = &event.kind else {
        if matches!(
            &event.kind,
            EventKind::Button(_) | EventKind::CaptureInterrupted
        ) {
            clear_transient_state();
        }
        return EventDisposition::Pass;
    };
    if key.state != PressState::Pressed || key.repeat {
        return EventDisposition::Pass;
    }
    let Some(shared) = ENGINE.get() else {
        return EventDisposition::Pass;
    };
    let Ok(mut state) = shared.lock() else {
        return EventDisposition::Pass;
    };

    if (key.modifiers.control || key.modifiers.meta)
        && key.modifiers.shift
        && character(key).is_some_and(|ch| ch.eq_ignore_ascii_case("z"))
    {
        if let Some(record) = state.undo.take() {
            if record.created.elapsed() <= Duration::from_secs(8)
                && replace_word(&record.corrected, &record.original, &record.delimiter)
            {
                state.word.clear();
                state.context.clear();
                let _ = state
                    .app
                    .emit("badelha-undo", serde_json::json!({ "restored": true }));
                return EventDisposition::Suppress;
            }
        }
    }

    if key.modifiers.control || key.modifiers.alt || key.modifiers.meta {
        state.word.clear();
        state.context.clear();
        state.undo = None;
        return EventDisposition::Pass;
    }

    if named(key, NamedKey::Backspace) {
        state.undo = None;
        state.word.pop();
        return EventDisposition::Pass;
    }
    if named(key, NamedKey::Enter) || named(key, NamedKey::Tab) || named(key, NamedKey::Escape) {
        state.word.clear();
        state.context.clear();
        state.undo = None;
        return EventDisposition::Pass;
    }

    let Some(typed) = character(key) else {
        return EventDisposition::Pass;
    };
    let is_delimiter = typed.chars().any(|ch| !ch.is_alphanumeric());
    if is_delimiter {
        state.undo = None;
        if !state.settings.enabled || state.word.is_empty() {
            state.word.clear();
            return EventDisposition::Pass;
        }
        let result = correct_token(
            &state.word,
            state.settings.confidence,
            &state.context,
            &state.settings.custom_words,
        );
        if let Some(correction) = result {
            let original = state.word.clone();
            if replace_word(&original, &correction.replacement, typed) {
                state.word.clear();
                state.context.push(correction.replacement.clone());
                state.context.truncate(4);
                state.undo = Some(UndoRecord {
                    original: original.clone(),
                    corrected: correction.replacement.clone(),
                    delimiter: typed.to_owned(),
                    created: Instant::now(),
                });
                let layout = if correction.language == "ar" {
                    "العربية"
                } else {
                    "English"
                };
                state.layout = Some(layout.into());
                if let Some(current) = CURRENT_LAYOUT.get() {
                    if let Ok(mut current) = current.lock() {
                        *current = Some(layout.into());
                    }
                }
                let _ = state.app.emit(
                    "badelha-correction",
                    serde_json::json!({
                        "from": original,
                        "to": correction.replacement,
                        "language": correction.language,
                        "confidence": correction.confidence,
                        "undoAvailable": true,
                        "undoShortcut": state.settings.undo_shortcut,
                        "undoExpiresInSeconds": 8
                    }),
                );
                if state.settings.switch_layout {
                    switch_layout();
                }
                return EventDisposition::Suppress;
            }
        }
        if !state.word.is_empty() {
            state.context.push(state.word.clone());
            state.context.truncate(4);
        }
        state.word.clear();
        return EventDisposition::Pass;
    }

    if typed.chars().all(|ch| ch.is_alphanumeric())
        && (typed.is_ascii()
            || typed
                .chars()
                .all(|ch| ('\u{0600}'..='\u{06ff}').contains(&ch)))
    {
        state.undo = None;
        if state.word.chars().count() < 64 {
            state.word.push_str(typed);
        } else {
            state.word.clear();
            state.context.clear();
        }
    } else {
        state.word.clear();
    }
    EventDisposition::Pass
}

fn clear_transient_state() {
    if let Some(engine) = ENGINE.get() {
        if let Ok(mut state) = engine.lock() {
            state.word.clear();
            state.context.clear();
            state.undo = None;
        }
    }
}

fn named(key: &KeyEvent, expected: NamedKey) -> bool {
    matches!(key.logical.as_ref(), Some(LogicalKey::Named(actual)) if *actual == expected)
}

fn character(key: &KeyEvent) -> Option<&str> {
    match key.logical.as_ref()? {
        LogicalKey::Character(value) => Some(value),
        _ => None,
    }
}

fn replace_word(original: &str, replacement: &str, delimiter: &str) -> bool {
    with_enigo(|enigo| {
        for _ in original.chars() {
            if enigo.key(Key::Backspace, Direction::Click).is_err() {
                return false;
            }
        }
        enigo.text(&format!("{replacement}{delimiter}")).is_ok()
    })
    .unwrap_or(false)
}

fn switch_layout() {
    let _ = with_enigo(|enigo| {
        #[cfg(target_os = "macos")]
        {
            let _ = enigo.key(Key::Control, Direction::Press);
            let _ = enigo.key(Key::Space, Direction::Click);
            let _ = enigo.key(Key::Control, Direction::Release);
        }
        #[cfg(target_os = "linux")]
        {
            let _ = enigo.key(Key::Alt, Direction::Press);
            let _ = enigo.key(Key::Shift, Direction::Press);
            let _ = enigo.key(Key::Shift, Direction::Release);
            let _ = enigo.key(Key::Alt, Direction::Release);
        }
    });
}

fn with_enigo<T>(f: impl FnOnce(&mut Enigo) -> T) -> Option<T> {
    static ENIGO: OnceLock<Mutex<Option<Enigo>>> = OnceLock::new();
    let lock = ENIGO.get_or_init(|| Mutex::new(Enigo::new(&EnigoSettings::default()).ok()));
    let mut slot = lock.lock().ok()?;
    f(slot.as_mut()?)
}
