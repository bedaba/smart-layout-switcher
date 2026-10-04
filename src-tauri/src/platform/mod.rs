use crate::{AppSettings, RuntimeStatus};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex, OnceLock},
};
use tauri::AppHandle;

#[derive(Clone, Debug)]
pub struct Correction {
    pub replacement: String,
    pub language: &'static str,
    pub confidence: f32,
}

pub trait PlatformInputAdapter {
    fn start(&self, settings: Arc<Mutex<AppSettings>>, app: AppHandle);
    fn status(&self) -> RuntimeStatus;
    fn undo_last_correction(&self) -> bool {
        false
    }
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::WindowsInputAdapter;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod portable;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use portable::PortableInputAdapter;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
struct UnsupportedInputAdapter;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
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
            message_en: "Keyboard monitoring and replacement are not implemented on this platform."
                .into(),
            layout: None,
        }
    }
}

#[cfg(target_os = "windows")]
fn adapter() -> impl PlatformInputAdapter {
    WindowsInputAdapter
}
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn adapter() -> impl PlatformInputAdapter {
    UnsupportedInputAdapter
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn adapter() -> impl PlatformInputAdapter {
    PortableInputAdapter
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
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    portable::set_settings(settings);
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let _ = settings;
}

pub fn undo_last_correction() -> bool {
    adapter().undo_last_correction()
}

pub fn convert_selection(text: &str) -> Option<String> {
    if text.is_empty() || text.chars().count() > 4096 || text.contains('@') {
        return None;
    }
    let mut source = None;
    let mut output = String::with_capacity(text.len());
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            token.push(ch);
            continue;
        }
        if !token.is_empty() {
            let token_script = script(&token)?;
            if source.is_some_and(|previous| !same_script(previous, token_script)) {
                return None;
            }
            source = Some(token_script);
            let (from, to) = match token_script {
                Script::Latin => (&ENGLISH_PROFILE, &ARABIC_PROFILE),
                Script::Arabic => (&ARABIC_PROFILE, &ENGLISH_PROFILE),
            };
            let mapped = map_between_profiles(&token, from, to)?;
            output.push_str(&preserve_case(&token, mapped, to.script));
            token.clear();
        }
        output.push(ch);
    }
    if !token.is_empty() {
        let token_script = script(&token)?;
        if source.is_some_and(|previous| !same_script(previous, token_script)) {
            return None;
        }
        let (from, to) = match token_script {
            Script::Latin => (&ENGLISH_PROFILE, &ARABIC_PROFILE),
            Script::Arabic => (&ARABIC_PROFILE, &ENGLISH_PROFILE),
        };
        let mapped = map_between_profiles(&token, from, to)?;
        output.push_str(&preserve_case(&token, mapped, to.script));
    }
    if output.trim().is_empty() || output.trim_end() == text.trim_end() {
        None
    } else {
        Some(output)
    }
}

fn same_script(left: Script, right: Script) -> bool {
    matches!(
        (left, right),
        (Script::Latin, Script::Latin) | (Script::Arabic, Script::Arabic)
    )
}

pub fn correct_token(
    input: &str,
    threshold: f32,
    context: &[String],
    custom_words: &[String],
) -> Option<Correction> {
    let script = script(input)?;
    let (source, target) = match script {
        Script::Latin => (&ENGLISH_PROFILE, &ARABIC_PROFILE),
        Script::Arabic => (&ARABIC_PROFILE, &ENGLISH_PROFILE),
    };
    let candidate = map_between_profiles(input, source, target)?;
    let original_score = language_score(input, source.script, context, custom_words);
    let candidate_score = language_score(&candidate, target.script, context, custom_words);
    let margin = candidate_score - original_score;
    if candidate_score < threshold.max(0.78) || margin < 0.16 {
        return None;
    }
    Some(Correction {
        replacement: preserve_case(input, candidate, target.script),
        language: target.id,
        confidence: candidate_score.clamp(0.0, 1.0),
    })
}

#[cfg(test)]
mod tests {
    use super::{convert_selection, correct_token};

    #[test]
    fn fixes_clear_arabic_layout_typo_to_english() {
        let correction = correct_token("اثممخ", 0.8, &[], &[]).expect("clear layout typo");
        assert_eq!(correction.replacement, "hello");
        assert_eq!(correction.language, "en");
        assert!(correction.confidence >= 0.8);
    }

    #[test]
    fn fixes_clear_english_layout_typo_to_arabic() {
        let correction = correct_token("sghl", 0.8, &[], &[]).expect("clear layout typo");
        assert_eq!(correction.replacement, "سلام");
        assert_eq!(correction.language, "ar");
    }

    #[test]
    fn leaves_correct_common_english_word_unchanged() {
        assert!(correct_token("hello", 0.8, &[], &[]).is_none());
    }

    #[test]
    fn custom_words_are_not_rewritten() {
        assert!(correct_token("Bedaba", 0.8, &[], &["Bedaba".into()]).is_none());
    }

    #[test]
    fn selected_text_with_email_is_left_unchanged() {
        assert!(convert_selection("hello@example.com").is_none());
    }

    #[test]
    fn mixed_language_selection_is_left_unchanged() {
        assert!(convert_selection("hello مرحبا").is_none());
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

fn language_score(word: &str, script: Script, context: &[String], custom_words: &[String]) -> f32 {
    if word.len() < 2 {
        return 0.0;
    }
    let normalized = normalize_word(word);
    if custom_words
        .iter()
        .any(|known| normalize_word(known) == normalized)
    {
        return 0.995;
    }
    let lexicon_match = dictionary_contains(&normalized, script);
    let chars: Vec<char> = normalized.chars().collect();
    if chars.len() < 3 {
        return if lexicon_match { 0.82 } else { 0.30 };
    }
    let (common, vowels) = match script {
        Script::Latin => ("th he in er an re on at en nd ti es or te of ed is it al ar st to nt ng se ha as ou io le ve co me de hi ri ro ic ne ea ra ce li ch ll be ma si om ur", "aeiouy"),
        Script::Arabic => ("المنفيتهعيربسرونيدكازملأو", ""),
    };
    let pair_score = pair_likelihood(&chars, common);
    let vowel_ratio = if vowels.is_empty() {
        0.0
    } else {
        chars.iter().filter(|c| vowels.contains(**c)).count() as f32 / chars.len() as f32
    };
    let trigram_score = trigram_likelihood(&chars, script);
    let script_valid = match script {
        Script::Arabic => chars.iter().all(|c| ('\u{0600}'..='\u{06ff}').contains(c)),
        Script::Latin => chars.iter().all(|c| c.is_ascii_alphabetic()),
    };
    if !script_valid {
        return 0.0;
    }
    let linguistic = 0.28 + pair_score * 0.30 + trigram_score * 0.20 + vowel_ratio * 0.12;
    let lexical = if lexicon_match {
        0.76 + frequency_bonus(&normalized, script) + context_bonus(&normalized, script, context)
    } else {
        0.0
    };
    linguistic.max(lexical).min(0.995)
}

fn dictionary_contains(word: &str, script: Script) -> bool {
    static ENGLISH: OnceLock<HashSet<String>> = OnceLock::new();
    static ARABIC: OnceLock<HashSet<String>> = OnceLock::new();
    let words = match script {
        Script::Latin => ENGLISH.get_or_init(|| {
            ENGLISH_WORDS
                .split_ascii_whitespace()
                .map(str::to_owned)
                .collect()
        }),
        Script::Arabic => ARABIC.get_or_init(|| {
            ARABIC_WORDS
                .split_whitespace()
                .map(normalize_word)
                .collect()
        }),
    };
    words.contains(word)
}

fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|ch| !matches!(*ch, '\u{064b}'..='\u{065f}' | '\u{0670}' | '\u{0640}'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn pair_likelihood(chars: &[char], common: &str) -> f32 {
    if chars.len() < 2 {
        return 0.0;
    }
    let familiar = chars
        .windows(2)
        .filter(|pair| {
            let pair = format!("{}{}", pair[0], pair[1]);
            common.split_ascii_whitespace().any(|known| known == pair)
        })
        .count();
    familiar as f32 / (chars.len() - 1) as f32
}

fn trigram_likelihood(chars: &[char], script: Script) -> f32 {
    if chars.len() < 3 {
        return 0.0;
    }
    let common = match script {
        Script::Latin => "the and ing her ere ent tha nth was eth for dth hat she ion tio ver ter est ers ati his all ith hes oft sth men",
        Script::Arabic => "المن انت الة يت اليا هاد فيا ير اتي الى من ا ان لة ون ا ت ن ا ل م و ا ي",
    };
    let hits = chars
        .windows(3)
        .filter(|window| {
            let tri = format!("{}{}{}", window[0], window[1], window[2]);
            common.split_ascii_whitespace().any(|known| known == tri)
        })
        .count();
    (hits as f32 / (chars.len() - 2) as f32).min(1.0)
}

fn frequency_bonus(word: &str, script: Script) -> f32 {
    let common = match script {
        Script::Latin => "the be to of and a in that have i it for not on with he as you do at this but his by from they we say her she or an will my one all would there their what so up out if about who get which go me when make can like time no just him know take people into year your good some could them see other than then now look only come its over think also back after use two how our work first well way even new want because any these give day most us",
        Script::Arabic => "في من على الى عن مع هذا هذه هو هي أنا انا انت أنت نحن كان كانت يكون لا ما لا الى إلى كل كما غير عند بعد قبل هنا هناك يوم اليوم يا لو مش في ده دي ده جدا نعم شكرا السلام الله الرحمن الرحيم مرحبا كيف صباح مساء عمل شغل بيت كتاب كلمة نص عربي انجليزي كان كنت قد يمكن عايز عايزة محتاج محتاجة تمام كويس",
    };
    if common.split_ascii_whitespace().any(|known| known == word) {
        0.16
    } else {
        0.06
    }
}

fn context_bonus(word: &str, script: Script, context: &[String]) -> f32 {
    let Some(previous) = context.last().map(|value| normalize_word(value)) else {
        return 0.0;
    };
    let pair = format!("{previous} {word}");
    let common = match script {
        Script::Latin => "hello world good morning thank you how are you have a in the on the at the please let me see you do it we are i am i have i want can you my name",
        Script::Arabic => "صباح الخير مساء الخير شكرا لك شكرا جزيلا ازيك يا انا عايز انا عايزة انا محتاج في البيت على فكرة لو سمحت من فضلك الحمد لله ان شاء الله كيف حالك",
    };
    if common
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|window| window.join(" ") == pair)
    {
        0.07
    } else {
        0.0
    }
}

const ENGLISH_WORDS: &str = "a about after again all also am an and any are as at back be because been before being best better between big both but by call came can come could day did do does doing down each even every find first for from get give go good great had has have he her here him his home how i if in into is it its just know last later like little long look made make many may me more most much must my new no not now of on one only or other our out over people place please point right said same say see she should show small some something still such take tell than that the their them then there these they thing think this those through time to too two under up us use very want was way we well went were what when where which who why will with work world would write written you your hello hi test words keyboard language switch typing automatic app application settings local private confidence arabic english enter space input profile before after hello world welcome thanks thank morning name text note message desktop computer program save close open account project number chapter around ask answer again always another apple area away baby back ball bank base be become began below black blue body book box boy build business buy car care case change child city class clear cold color country course cut dark data decision detail development different difficult door draw during early earth east easy eat education end enough environment example experience eye face fact family far fast father feel few field figure fill final financial fine fire fish five food foot form forward free friend full game general girl glass go government group grow hand happen hard has head hear help high hold hour house however idea important information interest issue job keep kind kitchen land large late learn least leave left less letter life light line list live local lost low lunch manager map mark material mean meet member minute mind miss money month morning mother move music must name nation near need never new next night north number office often order other own paper part pass people percent personal place plan play point power problem program provide public put question quick quite rain ready really reason right room run same school second see seem sentence set side simple since single sit skill small so something sometimes south speak special start state still stop study table talk teach team technology thank think third thing thought thousand thread through together tonight too took top toward town training travel tree try turn type under understand university until up update value very view voice wait walk want war water way we week well west what when where while white whole why wife will win with without woman word work world worry write wrong year yes yet you your you're didn't don't can't it's that's there's they're we've we'vell";
const ARABIC_WORDS: &str = "انا أنت انت انتي هو هي احنا نحن هم هما هذا هذه ذلك تلك هنا هناك الذي التي الذين ماذا لماذا كيف متى اين أين من ما في على عن الى إلى مع بين عند بعد قبل فوق تحت كل بعض غير جدا اكثر أكثر اقل أقل كان كانت يكون تكون كنت كنا كانوا عندي عندك عنده عندها عندنا عايز عاوز عايزة عاوزة اريد أريد ممكن مش ليس لا نعم اهلا أهلا مرحبا سلام السلام صباح مساء شكرا شكرًا لو سمحت معلش تمام كويس حلو جميل جديد قديم كبير صغير سريع بطيء سهل صعب مهم لازم لازمنا هنا هناك ابعت ابعث اكتب كتابة يكتب كتب اكتبها امسح احذف ابدأ ابدا ابدأها اخلص خلص خلينا نعمل اعمل يعمل تعمل نروح روح رايح رايحة جاي جايه هروح هعمل هكتب عنده عندها عندهم حاجة حاجه حاجات حاجة تانية تاني تانيه واحد واحدة اثنين ثلاثة اربعة خمسة عشرة يوم أيام شهر سنة وقت ساعة دقيقة ثانية دلوقتي الآن اليوم بكرة امبارح الصبح بالليل البيت الشغل المدرسة الجامعة البلد مصر عربي العربية انجليزي الإنجليزية اللغة لوحة مفاتيح كيبورد كلمة كلام جملة نص رسالة مشروع برنامج تطبيق تطبيقات سطح مكتب الكمبيوتر الجهاز ويندوز ماك لينكس حفظ افتح اقفل شغل تشغيل إيقاف الاعدادات إعدادات اختيار تشغيل تلقائي تلقائيا تلقائي محلي خصوصية بيانات كلمة مرور مرور ايميل البريد اسم ملف حساب مستخدم ممكن تقدر اقدر محتاج محتاجة عايز عايزة عايزين عندي عندك بعدين دلوقتي عشان علشان لكن لأن انا وانت وهي وهو شوية كل مرة حاجة غلط صح صحيح سليم سليمة مكتوب كتابة شوية علطول اوتوماتك تلقائي يحول حول اللغة الكيبورد keyboard";
