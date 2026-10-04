use crate::{AppSettings, RuntimeStatus};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

pub trait PlatformInputAdapter {
    fn start(&self, settings: Arc<Mutex<AppSettings>>, app: AppHandle);
    fn status(&self) -> RuntimeStatus;
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::WindowsInputAdapter;

#[cfg(not(target_os = "windows"))]
struct UnsupportedInputAdapter;

#[cfg(not(target_os = "windows"))]
impl PlatformInputAdapter for UnsupportedInputAdapter {
    fn start(&self, _settings: Arc<Mutex<AppSettings>>, _app: AppHandle) {}

    fn status(&self) -> RuntimeStatus {
        let platform = if cfg!(target_os = "macos") {
            "macOS"
        } else if cfg!(target_os = "linux") {
            "Linux"
        } else {
            "غير معروف"
        };
        RuntimeStatus {
            platform: platform.into(),
            supported: false,
            permission: "unsupported".into(),
            message: "واجهة مراقبة وإعادة إدخال لوحة المفاتيح لهذا النظام لم تُنفّذ بعد.".into(),
            layout: None,
        }
    }
}

#[cfg(target_os = "windows")]
fn adapter() -> impl PlatformInputAdapter {
    WindowsInputAdapter
}
#[cfg(not(target_os = "windows"))]
fn adapter() -> impl PlatformInputAdapter {
    UnsupportedInputAdapter
}

pub fn start(settings: Arc<Mutex<AppSettings>>, app: AppHandle) {
    adapter().start(settings, app);
}

pub fn status() -> RuntimeStatus {
    adapter().status()
}

pub fn set_settings(settings: AppSettings) {
    #[cfg(target_os = "windows")]
    windows::set_settings(settings);
    #[cfg(not(target_os = "windows"))]
    let _ = settings;
}

pub fn correct_token(input: &str, threshold: f32) -> Option<(String, &'static str)> {
    let script = script(input)?;
    let (source, target) = match script {
        Script::Latin => (&ENGLISH_PROFILE, &ARABIC_PROFILE),
        Script::Arabic => (&ARABIC_PROFILE, &ENGLISH_PROFILE),
    };
    let candidate = map_between_profiles(input, source, target)?;
    let original_score = language_score(input, source.script);
    let candidate_score = language_score(&candidate, target.script);
    if candidate_score >= threshold && candidate_score >= original_score + 0.12 {
        Some((preserve_case(input, candidate, target.script), target.id))
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum Script {
    Latin,
    Arabic,
}

fn script(input: &str) -> Option<Script> {
    if input.is_empty() || input.chars().count() > 64 {
        return None;
    }
    let mut latin = false;
    let mut arabic = false;
    for ch in input.chars() {
        if ch.is_ascii_alphabetic() {
            latin = true;
        } else if ('\u{0600}'..='\u{06ff}').contains(&ch) && ch.is_alphabetic() {
            arabic = true;
        } else {
            return None;
        }
    }
    match (latin, arabic) {
        (true, false) => Some(Script::Latin),
        (false, true) => Some(Script::Arabic),
        _ => None,
    }
}

struct LayoutProfile {
    id: &'static str,
    script: Script,
    keys: &'static str,
    glyphs: &'static [&'static str],
}

const ENGLISH_PROFILE: LayoutProfile = LayoutProfile {
    id: "en",
    script: Script::Latin,
    keys: "qwertyuiop[]asdfghjkl;'zxcvbnm,./",
    glyphs: &[
        "q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "[", "]", "a", "s", "d", "f", "g", "h",
        "j", "k", "l", ";", "'", "z", "x", "c", "v", "b", "n", "m", ",", ".", "/",
    ],
};
const ARABIC_PROFILE: LayoutProfile = LayoutProfile {
    id: "ar",
    script: Script::Arabic,
    keys: "qwertyuiop[]asdfghjkl;'zxcvbnm,./",
    glyphs: &[
        "ض", "ص", "ث", "ق", "ف", "غ", "ع", "ه", "خ", "ح", "ج", "د", "ش", "س", "ي", "ب", "ل", "ا",
        "ت", "ن", "م", "ك", "ط", "ئ", "ء", "ؤ", "ر", "لا", "ى", "ة", "و", "ز", "ظ",
    ],
};

fn map_between_profiles(
    input: &str,
    source: &LayoutProfile,
    target: &LayoutProfile,
) -> Option<String> {
    if source.keys.chars().count() != source.glyphs.len()
        || target.keys.chars().count() != target.glyphs.len()
    {
        return None;
    }
    let normalized = input.to_lowercase();
    let mut cursor = normalized.as_str();
    let mut out = String::new();
    while !cursor.is_empty() {
        let (index, glyph) = source
            .glyphs
            .iter()
            .enumerate()
            .filter(|(_, candidate)| cursor.starts_with(**candidate))
            .max_by_key(|(_, candidate)| candidate.len())?;
        out.push_str(*target.glyphs.get(index)?);
        cursor = &cursor[glyph.len()..];
    }
    Some(out)
}

fn preserve_case(input: &str, mut candidate: String, target: Script) -> String {
    if matches!(target, Script::Latin) && input.chars().any(|c| c.is_ascii_alphabetic()) {
        if input
            .chars()
            .all(|c| !c.is_ascii_alphabetic() || c.is_ascii_uppercase())
        {
            candidate.make_ascii_uppercase();
        } else if input.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            let mut chars = candidate.chars();
            if let Some(first) = chars.next() {
                let rest: String = chars.collect();
                candidate = first.to_ascii_uppercase().to_string() + &rest;
            }
        }
    }
    candidate
}

fn language_score(word: &str, script: Script) -> f32 {
    if word.len() < 2 {
        return 0.0;
    }
    if dictionary_contains(word, script) {
        return 0.99;
    }
    let chars: Vec<char> = word.to_lowercase().chars().collect();
    if chars.len() < 3 {
        return 0.35;
    }
    let (common, vowels) = match script {
        Script::Latin => ("th he in er an re on at en nd ti es or te of ed is it al ar st to nt ng se ha as ou io le ve co me de hi ri ro ic ne ea ra ce li ch ll be ma si om ur", "aeiouy"),
        Script::Arabic => ("المنفيتهعيربسرونيدكازملأو", ""),
    };
    let mut familiar = 0.0;
    let mut pairs: f32 = 0.0;
    for pair in chars.windows(2) {
        pairs += 1.0;
        if common.contains(pair[0]) && common.contains(pair[1]) {
            familiar += 1.0;
        }
    }
    let vowel_ratio = if vowels.is_empty() {
        0.0
    } else {
        chars.iter().filter(|c| vowels.contains(**c)).count() as f32 / chars.len() as f32
    };
    let script_score = if matches!(script, Script::Arabic)
        && chars.iter().all(|c| ('\u{0600}'..='\u{06ff}').contains(c))
        || matches!(script, Script::Latin) && chars.iter().all(|c| c.is_ascii_alphabetic())
    {
        0.35
    } else {
        0.0
    };
    (script_score + (familiar / pairs.max(1.0)) * 0.26 + vowel_ratio * 0.22).min(0.82)
}

fn dictionary_contains(word: &str, script: Script) -> bool {
    let normalized = word.to_lowercase();
    match script {
        Script::Latin => ENGLISH_WORDS
            .split_ascii_whitespace()
            .any(|candidate| candidate == normalized),
        Script::Arabic => ARABIC_WORDS
            .split_ascii_whitespace()
            .any(|candidate| candidate == normalized),
    }
}

const ENGLISH_WORDS: &str = "a about after again all also am an and any are as at back be because been before being best better between big both but by call came can come could day did do does doing down each even every find first for from get give go good great had has have he her here him his home how i if in into is it its just know last later like little long look made make many may me more most much must my new no not now of on one only or other our out over people place please point right said same say see she should show small some something still such take tell than that the their them then there these they thing think this those through time to too two under up us use very want was way we well went were what when where which who why will with work world would write written you your hello hi test words keyboard language switch typing automatic app application settings local private confidence arabic english enter space input profile before after hello world welcome thanks thank morning name text note message desktop computer program save close open account project number chapter around ask answer again always another apple area away baby back ball bank base be become began below black blue body book box boy build business buy car care case change child city class clear cold color country course cut dark data decision detail development different difficult door draw during early earth east easy eat education end enough environment example experience eye face fact family far fast father feel few field figure fill final financial fine fire fish five food foot form forward free friend full game general girl glass go government group grow hand happen hard has head hear help high hold hour house however idea important information interest issue job keep kind kitchen land large late learn least leave left less letter life light line list live local lost low lunch manager map mark material mean meet member minute mind miss money month morning mother move music must name nation near need never new next night north number office often order other own paper part pass people percent personal place plan play point power problem program provide public put question quick quite rain ready really reason right room run same school second see seem sentence set side simple since single sit skill small so something sometimes south speak special start state still stop study table talk teach team technology thank think third thing thought thousand thread through together tonight too took top toward town training travel tree try turn type under understand university until up update value very view voice wait walk want war water way we week well west what when where while white whole why wife will win with without woman word work world worry write wrong year yes yet you your you're didn't don't can't it's that's there's they're we've we'vell";
const ARABIC_WORDS: &str = "انا أنت انت انتي هو هي احنا نحن هم هما هذا هذه ذلك تلك هنا هناك الذي التي الذين ماذا لماذا كيف متى اين أين من ما في على عن الى إلى مع بين عند بعد قبل فوق تحت كل بعض غير جدا اكثر أكثر اقل أقل كان كانت يكون تكون كنت كنا كانوا عندي عندك عنده عندها عندنا عايز عاوز عايزة عاوزة اريد أريد ممكن مش ليس لا نعم اهلا أهلا مرحبا صباح مساء شكرا شكرًا لو سمحت معلش تمام كويس حلو جميل جديد قديم كبير صغير سريع بطيء سهل صعب مهم لازم لازمنا هنا هناك ابعت ابعث اكتب كتابة يكتب كتب اكتبها امسح احذف ابدأ ابدا ابدأها اخلص خلص خلينا نعمل اعمل يعمل تعمل نروح روح رايح رايحة جاي جايه هروح هعمل هكتب عنده عندها عندهم حاجة حاجه حاجات حاجة تانية تاني تانيه واحد واحدة اثنين ثلاثة اربعة خمسة عشرة يوم أيام شهر سنة وقت ساعة دقيقة ثانية دلوقتي الآن اليوم بكرة امبارح الصبح بالليل البيت الشغل المدرسة الجامعة البلد مصر عربي العربية انجليزي الإنجليزية اللغة لوحة مفاتيح كيبورد كلمة كلام جملة نص رسالة مشروع برنامج تطبيق تطبيقات سطح مكتب الكمبيوتر الجهاز ويندوز ماك لينكس حفظ افتح اقفل شغل تشغيل إيقاف الاعدادات إعدادات اختيار تشغيل تلقائي تلقائيا تلقائي محلي خصوصية بيانات كلمة مرور مرور ايميل البريد اسم ملف حساب مستخدم ممكن تقدر اقدر محتاج محتاجة عايز عايزة عايزين عندي عندك بعدين دلوقتي عشان علشان لكن لأن انا وانت وهي وهو شوية كل مرة حاجة غلط صح صحيح سليم سليمة مكتوب كتابة شوية علطول اوتوماتك تلقائي يحول حول اللغة الكيبورد keyboard";
