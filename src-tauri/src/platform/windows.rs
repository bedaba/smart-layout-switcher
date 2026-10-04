use super::{correct_token, PlatformInputAdapter};
use crate::{AppSettings, RuntimeStatus};
use std::{
    mem::{size_of, zeroed},
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
};
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::{
        Controls::EM_GETPASSWORDCHAR,
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyboardLayout, GetKeyboardLayoutList, GetKeyboardState,
            SendInput, ToUnicodeEx, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            KEYEVENTF_UNICODE, VK_BACK, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo,
            GetMessageW, GetWindowThreadProcessId, PostMessageW, SendMessageW, SetWindowsHookExW,
            TranslateMessage, UnhookWindowsHookEx, GUITHREADINFO, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
            MSG, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_INPUTLANGCHANGEREQUEST, WM_KEYDOWN, WM_SYSKEYDOWN,
        },
    },
};

struct EngineState {
    settings: AppSettings,
    word: String,
    foreground: isize,
    app: AppHandle,
}

static ENGINE: OnceLock<Arc<Mutex<EngineState>>> = OnceLock::new();
static HOOK_READY: AtomicBool = AtomicBool::new(false);
static CURRENT_LAYOUT: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub struct WindowsInputAdapter;

impl PlatformInputAdapter for WindowsInputAdapter {
    fn start(&self, settings: Arc<Mutex<AppSettings>>, app: AppHandle) {
        let initial = settings.lock().map(|s| s.clone()).unwrap_or_default();
        let _ = ENGINE.set(Arc::new(Mutex::new(EngineState {
            settings: initial,
            word: String::new(),
            foreground: 0,
            app,
        })));
        thread::Builder::new()
            .name("badelha-keyboard-hook".into())
            .spawn(hook_thread)
            .ok();
    }

    fn status(&self) -> RuntimeStatus {
        let layout = CURRENT_LAYOUT
            .get_or_init(|| Mutex::new(None))
            .lock()
            .ok()
            .and_then(|v| v.clone());
        let ready = HOOK_READY.load(Ordering::Relaxed);
        RuntimeStatus {
            platform: "Windows".into(),
            supported: ready,
            permission: if ready {
                "ready".into()
            } else {
                "unsupported".into()
            },
            message: if ready {
                "التصحيح محلي. يتجاهل حقول كلمات المرور القياسية والتطبيقات المستثناة.".into()
            } else {
                "تعذر تشغيل مراقب لوحة المفاتيح. أعد فتح التطبيق أو شغّله بنفس مستوى صلاحية البرنامج الذي تكتب فيه.".into()
            },
            layout,
        }
    }
}

pub fn set_settings(settings: AppSettings) {
    if let Some(engine) = ENGINE.get() {
        if let Ok(mut state) = engine.lock() {
            state.settings = settings;
        }
    }
}

fn hook_thread() {
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), null_mut(), 0);
        if hook.is_null() {
            return;
        }
        let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), null_mut(), 0);
        if mouse_hook.is_null() {
            UnhookWindowsHookEx(hook);
            return;
        }
        HOOK_READY.store(true, Ordering::Relaxed);
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        UnhookWindowsHookEx(hook);
        UnhookWindowsHookEx(mouse_hook);
        HOOK_READY.store(false, Ordering::Relaxed);
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(shared) = ENGINE.get() {
            if let Ok(mut state) = shared.lock() {
                state.word.clear();
            }
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 || (wparam != WM_KEYDOWN as usize && wparam != WM_SYSKEYDOWN as usize) {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let event = &*(lparam as *const KBDLLHOOKSTRUCT);
    if event.flags & LLKHF_INJECTED != 0 {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let Some(shared) = ENGINE.get() else {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    };
    let Ok(mut state) = shared.lock() else {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    };
    let foreground = GetForegroundWindow() as isize;
    if foreground != state.foreground {
        state.word.clear();
        state.foreground = foreground;
    }
    if GetAsyncKeyState(VK_CONTROL as i32) < 0
        || GetAsyncKeyState(VK_MENU as i32) < 0
        || GetAsyncKeyState(VK_LWIN as i32) < 0
        || GetAsyncKeyState(VK_RWIN as i32) < 0
    {
        state.word.clear();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if event.vkCode == VK_BACK as u32 {
        state.word.pop();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    if event.vkCode == 0x1b || event.vkCode == 0x0d {
        state.word.clear();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    let Some((typed, layout_id)) = typed_character(event) else {
        if event.vkCode != 0x10
            && event.vkCode != 0x11
            && event.vkCode != 0x12
            && event.vkCode != 0xA0
            && event.vkCode != 0xA1
            && event.vkCode != 0xA2
            && event.vkCode != 0xA3
            && event.vkCode != 0xA4
            && event.vkCode != 0xA5
        {
            state.word.clear();
        }
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    };
    if let Some(cell) = CURRENT_LAYOUT.get() {
        if let Ok(mut current) = cell.lock() {
            *current = Some(if layout_id & 0x03ff == 0x01 {
                "العربية".into()
            } else if layout_id & 0x03ff == 0x09 {
                "English".into()
            } else {
                "أخرى".into()
            });
        }
    }

    let is_delimiter = event.vkCode == 0x20 || typed.chars().any(|ch| !ch.is_alphanumeric());
    if is_delimiter {
        if !state.settings.enabled
            || state.word.is_empty()
            || is_password_field()
            || is_excluded(&state.settings.excluded_apps)
        {
            state.word.clear();
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        let threshold = state.settings.confidence;
        let result = correct_token(&state.word, threshold);
        if let Some((replacement, target_layout)) = result {
            let old = state.word.clone();
            let delimiter = if event.vkCode == 0x20 {
                " "
            } else {
                typed.as_str()
            };
            let sent = replace_word(&old, &replacement, delimiter);
            state.word.clear();
            if sent {
                if state.settings.switch_layout {
                    switch_layout(target_layout);
                }
                let _ = state.app.emit("badelha-correction", serde_json::json!({ "from": old, "to": replacement, "language": target_layout }));
                return 1;
            }
        }
        state.word.clear();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if typed.chars().all(|ch| ch.is_alphanumeric())
        && (typed.is_ascii()
            || typed
                .chars()
                .all(|ch| ('\u{0600}'..='\u{06ff}').contains(&ch)))
    {
        if state.word.chars().count() < 64 {
            state.word.push_str(&typed);
        } else {
            state.word.clear();
        }
    } else {
        state.word.clear();
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

unsafe fn typed_character(event: &KBDLLHOOKSTRUCT) -> Option<(String, u16)> {
    let hwnd: HWND = GetForegroundWindow();
    if hwnd.is_null() {
        return None;
    }
    let thread_id = GetWindowThreadProcessId(hwnd, null_mut());
    let layout = GetKeyboardLayout(thread_id);
    let language = (layout as usize & 0xffff) as u16;
    let mut keyboard = [0_u8; 256];
    if GetKeyboardState(keyboard.as_mut_ptr()) == 0 {
        return None;
    }
    if (event.vkCode as usize) < keyboard.len() {
        keyboard[event.vkCode as usize] |= 0x80;
    }
    let mut buffer = [0_u16; 8];
    let result = ToUnicodeEx(
        event.vkCode,
        event.scanCode,
        keyboard.as_ptr(),
        buffer.as_mut_ptr(),
        buffer.len() as i32,
        0,
        layout,
    );
    if result <= 0 {
        return None;
    }
    let text = String::from_utf16(&buffer[..result as usize]).ok()?;
    if text.is_empty() {
        return None;
    }
    let name = if language & 0x03ff == 0x01 {
        "ar"
    } else if language == 0x0409 {
        "en"
    } else {
        "other"
    };
    Some((
        text,
        if name == "ar" {
            0x0401
        } else if name == "en" {
            0x0409
        } else {
            language
        },
    ))
}

fn replace_word(original: &str, replacement: &str, delimiter: &str) -> bool {
    let mut inputs = Vec::new();
    for _ in 0..original.chars().count() {
        push_key(&mut inputs, VK_BACK as u16, 0);
    }
    for unit in replacement.encode_utf16() {
        push_key(&mut inputs, 0, unit);
    }
    for unit in delimiter.encode_utf16() {
        push_key(&mut inputs, 0, unit);
    }
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        ) == inputs.len() as u32
    }
}

fn push_key(inputs: &mut Vec<INPUT>, vk: u16, scan: u16) {
    let down_flags = if vk == 0 { KEYEVENTF_UNICODE } else { 0 };
    inputs.push(INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: down_flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    });
    inputs.push(INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: down_flags | KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    });
}

unsafe fn switch_layout(language: &str) {
    let wanted = if language == "ar" {
        0x0401_u16
    } else {
        0x0409_u16
    };
    let mut layouts = [null_mut(); 32];
    let count = GetKeyboardLayoutList(layouts.len() as i32, layouts.as_mut_ptr());
    if count <= 0 {
        return;
    }
    let target = layouts[..(count as usize).min(layouts.len())]
        .iter()
        .find(|hkl| {
            let lang = (**hkl as usize & 0xffff) as u16;
            (lang & 0x03ff) == (wanted & 0x03ff)
        });
    if let Some(hkl) = target {
        let hwnd = GetForegroundWindow();
        if !hwnd.is_null() {
            PostMessageW(hwnd, WM_INPUTLANGCHANGEREQUEST, 0, *hkl as LPARAM);
        }
    }
}

unsafe fn is_password_field() -> bool {
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return false;
    }
    let tid = GetWindowThreadProcessId(hwnd, null_mut());
    let mut info: GUITHREADINFO = zeroed();
    info.cbSize = size_of::<GUITHREADINFO>() as u32;
    if GetGUIThreadInfo(tid, &mut info) == 0 || info.hwndFocus.is_null() {
        return false;
    }
    let mut class = [0_u16; 128];
    let length = GetClassNameW(info.hwndFocus, class.as_mut_ptr(), class.len() as i32);
    if length <= 0 {
        return false;
    }
    let name = String::from_utf16_lossy(&class[..length as usize]).to_lowercase();
    if !name.contains("edit") {
        return false;
    }
    SendMessageW(info.hwndFocus, EM_GETPASSWORDCHAR as u32, 0, 0) != 0
}

unsafe fn is_excluded(excluded: &[String]) -> bool {
    if excluded.is_empty() {
        return false;
    }
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return false;
    }
    let mut pid = 0_u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 {
        return false;
    }
    let process: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return false;
    }
    let mut path = [0_u16; 32768];
    let mut len = path.len() as u32;
    let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len) != 0;
    CloseHandle(process);
    if !ok {
        return false;
    }
    let full = String::from_utf16_lossy(&path[..len as usize]).to_lowercase();
    let name = full.rsplit(['\\', '/']).next().unwrap_or(&full);
    excluded.iter().any(|item| item == name)
}
