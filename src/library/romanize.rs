//! Zero-dependency, ultra-fast Korean (Hangul Revised Romanization) and
//! Japanese (Hiragana & Katakana Hepburn Romaji) script detector and romanizer.
//!
//! Handles:
//! - Hangul Syllables (U+AC00..=U+D7A3) via standard Unicode mathematical decomposition
//! - Hangul Jamo (U+1100..=U+11FF, U+3130..=U+318F)
//! - Hiragana (U+3040..=U+309F) and Katakana (U+30A0..=U+30FF) with sokuon (small tsu),
//!   yoon (combinations: kyo, sha, cha, etc.), and voiced marks (dakuten/handakuten).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    Korean,
    Japanese,
    Mixed,
    None,
}

pub struct ScriptDetector;

impl ScriptDetector {
    pub fn is_hangul(c: char) -> bool {
        matches!(c,
            '\u{AC00}'..='\u{D7A3}' | // Hangul Syllables
            '\u{1100}'..='\u{11FF}' | // Hangul Jamo
            '\u{3130}'..='\u{318F}'   // Hangul Compatibility Jamo
        )
    }

    pub fn is_hiragana(c: char) -> bool {
        matches!(c, '\u{3040}'..='\u{309F}')
    }

    pub fn is_katakana(c: char) -> bool {
        matches!(c, '\u{30A0}'..='\u{30FF}')
    }

    pub fn is_kanji(c: char) -> bool {
        matches!(c,
            '\u{4E00}'..='\u{9FFF}' | // CJK Unified Ideographs
            '\u{3400}'..='\u{4DBF}' | // CJK Unified Ideographs Extension A
            '\u{F900}'..='\u{FAFF}'   // CJK Compatibility Ideographs
        )
    }

    pub fn detect_script(text: &str) -> ScriptType {
        let mut has_korean = false;
        let mut has_japanese = false;

        for c in text.chars() {
            if Self::is_hangul(c) {
                has_korean = true;
            } else if Self::is_hiragana(c) || Self::is_katakana(c) {
                has_japanese = true;
            }
        }

        if has_korean && has_japanese {
            ScriptType::Mixed
        } else if has_korean {
            ScriptType::Korean
        } else if has_japanese {
            ScriptType::Japanese
        } else {
            ScriptType::None
        }
    }

    pub fn contains_cjk(text: &str) -> bool {
        text.chars().any(|c| Self::is_hangul(c) || Self::is_hiragana(c) || Self::is_katakana(c) || Self::is_kanji(c))
    }
}

// =========================================================================
// Korean Hangul Revised Romanization (RR Standard)
// =========================================================================

const HANGUL_BASE: u32 = 0xAC00;
const HANGUL_LAST: u32 = 0xD7A3;

// Initial consonants (Choseong, 19)
const CHOSEONG: [&str; 19] = [
    "g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s",
    "ss", "", "j", "jj", "ch", "k", "t", "p", "h",
];

// Medial vowels (Jungseong, 21)
const JUNGSEONG: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa",
    "wae", "oe", "yo", "u", "wo", "we", "wi", "yu", "eu", "ui", "i",
];

// Final consonants (Jongseong, 28)
const JONGSEONG: [&str; 28] = [
    "", "k", "k", "ks", "n", "nj", "nh", "t", "l", "lg",
    "lm", "lb", "ls", "lt", "lp", "lh", "m", "p", "bs", "s",
    "ss", "ng", "t", "t", "k", "t", "p", "h",
];

pub fn romanize_korean(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(text.len() * 2);
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];
        let code = c as u32;
        if (HANGUL_BASE..=HANGUL_LAST).contains(&code) {
            let offset = code - HANGUL_BASE;
            let cho_idx = (offset / (21 * 28)) as usize;
            let jung_idx = ((offset % (21 * 28)) / 28) as usize;
            let jong_idx = (offset % 28) as usize;

            let mut cho = CHOSEONG.get(cho_idx).copied().unwrap_or("");
            let jung = JUNGSEONG.get(jung_idx).copied().unwrap_or("");
            let mut jong = JONGSEONG.get(jong_idx).copied().unwrap_or("");

            // Assimilation check with previous syllable final consonant:
            // In Revised Romanization: ㄹ (l) followed by ㄹ becomes ll.
            // When ㄹ is followed by a vowel or ㅇ (silent), it is 'r'.
            // When ㄹ is followed by another consonant or at end of word, it is 'l'.
            // If previous syllable ended in ㄹ ('l') and current syllable starts with ㄹ ('r'),
            // they assimilate to "ll" (e.g. 아일릿: 일 (il) + 릿 (lit -> r/l initial)).
            if i > 0 {
                let prev_c = chars[i - 1];
                let prev_code = prev_c as u32;
                if (HANGUL_BASE..=HANGUL_LAST).contains(&prev_code) {
                    let prev_jong = (prev_code - HANGUL_BASE) % 28;
                    // prev_jong 8 is ㄹ ('l')
                    if prev_jong == 8 && cho_idx == 5 {
                        // ㄹ + ㄹ -> 'l'
                        cho = "l";
                    }
                }
            }

            // Word-final or syllable-final ㅅ (s) before consonant/end is pronounced 't' (e.g. 릿 -> lit)
            if jong_idx == 19 {
                // Check if next char is vowel (ㅇ)
                let next_is_vowel = if i + 1 < len {
                    let next_code = chars[i + 1] as u32;
                    if (HANGUL_BASE..=HANGUL_LAST).contains(&next_code) {
                        let next_cho = ((next_code - HANGUL_BASE) / (21 * 28)) as usize;
                        next_cho == 11 // ㅇ (null initial)
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !next_is_vowel {
                    jong = "t";
                }
            }

            result.push_str(cho);
            result.push_str(jung);
            result.push_str(jong);
        } else {
            // Keep original character (spaces, punctuation, latin)
            result.push(c);
        }
    }

    result
}

// =========================================================================
// Japanese Hiragana & Katakana Romaji (Hepburn Standard)
// =========================================================================

pub fn romanize_japanese(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(text.len() * 2);
    let mut i = 0;
    let len = chars.len();

    while i < len {
        let c = chars[i];

        // 1. Sokuon (small tsu: っ / ッ) -> doubles next consonant
        if (c == 'っ' || c == 'ッ') && i + 1 < len {
            let next_c = chars[i + 1];
            if let Some(next_romaji) = kana_to_romaji(next_c) {
                if let Some(first_char) = next_romaji.chars().next() {
                    // "ch" becomes "tch" in Hepburn
                    if next_romaji.starts_with("ch") {
                        result.push('t');
                    } else if first_char.is_ascii_alphabetic() {
                        result.push(first_char);
                    }
                    i += 1;
                    continue;
                }
            }
        }

        // 2. Check 2-character combo (Digraphs: きゃ kya, しゃ sha, ちゃ cha, etc.)
        if i + 1 < len {
            let next_c = chars[i + 1];
            if is_small_kana(next_c) {
                if let Some(combo_str) = kana_combo_to_romaji(c, next_c) {
                    result.push_str(combo_str);
                    i += 2;
                    // In Hepburn romanization, 'ou' or 'oo' after 'kyo' is often 'o' or 'ou'
                    // If next char is う or ウ after an 'o' combo (like きょう), we can either keep or merge.
                    // For music search/display, kyo or kyou both match; let's handle long o (う/ウ):
                    if i < len && (chars[i] == 'う' || chars[i] == 'ウ') {
                        // Skip the elongation vowel in Hepburn
                        i += 1;
                    }
                    continue;
                }
            }
        }

        // 3. Single kana
        if let Some(romaji) = kana_to_romaji(c) {
            result.push_str(romaji);
        } else {
            result.push(c);
        }

        i += 1;
    }

    result
}

fn is_small_kana(c: char) -> bool {
    matches!(c,
        'ゃ' | 'ゅ' | 'ょ' | 'ぁ' | 'ぃ' | 'ぅ' | 'ぇ' | 'ぉ' |
        'ャ' | 'ュ' | 'ョ' | 'ァ' | 'ィ' | 'ゥ' | 'ェ' | 'ォ'
    )
}

fn kana_combo_to_romaji(base: char, small: char) -> Option<&'static str> {
    match (base, small) {
        // Hiragana Combos
        ('き', 'ゃ') => Some("kya"), ('き', 'ゅ') => Some("kyu"), ('き', 'ょ') => Some("kyo"),
        ('ぎ', 'ゃ') => Some("gya"), ('ぎ', 'ゅ') => Some("gyu"), ('ぎ', 'ょ') => Some("gyo"),
        ('し', 'ゃ') => Some("sha"), ('し', 'ゅ') => Some("shu"), ('し', 'ょ') => Some("sho"),
        ('じ', 'ゃ') => Some("ja"),  ('じ', 'ゅ') => Some("ju"),  ('じ', 'ょ') => Some("jo"),
        ('ち', 'ゃ') => Some("cha"), ('ち', 'ゅ') => Some("chu"), ('ち', 'ょ') => Some("cho"),
        ('に', 'ゃ') => Some("nya"), ('に', 'ゅ') => Some("nyu"), ('に', 'ょ') => Some("nyo"),
        ('ひ', 'ゃ') => Some("hya"), ('ひ', 'ゅ') => Some("hyu"), ('ひ', 'ょ') => Some("hyo"),
        ('び', 'ゃ') => Some("bya"), ('び', 'ゅ') => Some("byu"), ('び', 'ょ') => Some("byo"),
        ('ぴ', 'ゃ') => Some("pya"), ('ぴ', 'ゅ') => Some("pyu"), ('ぴ', 'ょ') => Some("pyo"),
        ('み', 'ゃ') => Some("mya"), ('み', 'ゅ') => Some("myu"), ('み', 'ょ') => Some("myo"),
        ('り', 'ゃ') => Some("rya"), ('り', 'ゅ') => Some("ryu"), ('り', 'ょ') => Some("ryo"),

        // Katakana Combos
        ('キ', 'ャ') => Some("kya"), ('キ', 'ュ') => Some("kyu"), ('キ', 'ョ') => Some("kyo"),
        ('ギ', 'ャ') => Some("gya"), ('ギ', 'ュ') => Some("gyu"), ('ギ', 'ョ') => Some("gyo"),
        ('シ', 'ャ') => Some("sha"), ('シ', 'ュ') => Some("shu"), ('シ', 'ョ') => Some("sho"),
        ('ジ', 'ャ') => Some("ja"),  ('ジ', 'ュ') => Some("ju"),  ('ジ', 'ョ') => Some("jo"),
        ('チ', 'ャ') => Some("cha"), ('チ', 'ュ') => Some("chu"), ('チ', 'ョ') => Some("cho"),
        ('ニ', 'ャ') => Some("nya"), ('ニ', 'ュ') => Some("nyu"), ('ニ', 'ョ') => Some("nyo"),
        ('ヒ', 'ャ') => Some("hya"), ('ヒ', 'ュ') => Some("hyu"), ('ヒ', 'ョ') => Some("hyo"),
        ('ビ', 'ャ') => Some("bya"), ('ビ', 'ュ') => Some("byu"), ('ビ', 'ョ') => Some("byo"),
        ('ピ', 'ャ') => Some("pya"), ('ピ', 'ュ') => Some("pyu"), ('ピ', 'ョ') => Some("pyo"),
        ('ミ', 'ャ') => Some("mya"), ('ミ', 'ュ') => Some("myu"), ('ミ', 'ョ') => Some("myo"),
        ('リ', 'ャ') => Some("rya"), ('リ', 'ュ') => Some("ryu"), ('リ', 'ョ') => Some("ryo"),

        // Modern Katakana combos (ti, di, tu, fa, fi, etc.)
        ('テ', 'ィ') => Some("ti"),  ('デ', 'ィ') => Some("di"),
        ('フ', 'ァ') => Some("fa"),  ('フ', 'ィ') => Some("fi"),  ('フ', 'ェ') => Some("fe"), ('フ', 'ォ') => Some("fo"),
        ('ヴ', 'ァ') => Some("va"),  ('ヴ', 'ィ') => Some("vi"),  ('ヴ', 'ェ') => Some("ve"), ('ヴ', 'ォ') => Some("vo"),
        _ => None,
    }
}

fn kana_to_romaji(c: char) -> Option<&'static str> {
    match c {
        // Hiragana Vowels
        'あ' | 'ア' => Some("a"),
        'い' | 'イ' => Some("i"),
        'う' | 'ウ' => Some("u"),
        'え' | 'エ' => Some("e"),
        'お' | 'オ' => Some("o"),

        // K-line
        'か' | 'カ' => Some("ka"),
        'き' | 'キ' => Some("ki"),
        'く' | 'ク' => Some("ku"),
        'け' | 'ケ' => Some("ke"),
        'こ' | 'コ' => Some("ko"),

        // G-line (Dakuten)
        'が' | 'ガ' => Some("ga"),
        'ぎ' | 'ギ' => Some("gi"),
        'ぐ' | 'グ' => Some("gu"),
        'げ' | 'ゲ' => Some("ge"),
        'ご' | 'ゴ' => Some("go"),

        // S-line
        'さ' | 'サ' => Some("sa"),
        'し' | 'シ' => Some("shi"),
        'す' | 'ス' => Some("su"),
        'せ' | 'セ' => Some("se"),
        'そ' | 'ソ' => Some("so"),

        // Z/J-line (Dakuten)
        'ざ' | 'ザ' => Some("za"),
        'じ' | 'ジ' => Some("ji"),
        'ず' | 'ズ' => Some("zu"),
        'ぜ' | 'ゼ' => Some("ze"),
        'ぞ' | 'ゾ' => Some("zo"),

        // T-line
        'た' | 'タ' => Some("ta"),
        'ち' | 'チ' => Some("chi"),
        'つ' | 'ツ' => Some("tsu"),
        'て' | 'テ' => Some("te"),
        'と' | 'ト' => Some("to"),

        // D-line (Dakuten)
        'だ' | 'ダ' => Some("da"),
        'ぢ' | 'ヂ' => Some("ji"),
        'づ' | 'ヅ' => Some("zu"),
        'で' | 'デ' => Some("de"),
        'ど' | 'ド' => Some("do"),

        // N-line
        'な' | 'ナ' => Some("na"),
        'に' | 'ニ' => Some("ni"),
        'ぬ' | 'ヌ' => Some("nu"),
        'ね' | 'ネ' => Some("ne"),
        'の' | 'ノ' => Some("no"),

        // H-line
        'は' | 'ハ' => Some("ha"),
        'ひ' | 'ヒ' => Some("hi"),
        'ふ' | 'フ' => Some("fu"),
        'へ' | 'ヘ' => Some("he"),
        'ほ' | 'ホ' => Some("ho"),

        // B-line (Dakuten)
        'ば' | 'バ' => Some("ba"),
        'び' | 'ビ' => Some("bi"),
        'ぶ' | 'ブ' => Some("bu"),
        'べ' | 'ベ' => Some("be"),
        'ぼ' | 'ボ' => Some("bo"),

        // P-line (Handakuten)
        'ぱ' | 'パ' => Some("pa"),
        'ぴ' | 'ピ' => Some("pi"),
        'ぷ' | 'プ' => Some("pu"),
        'ぺ' | 'ペ' => Some("pe"),
        'ぽ' | 'ポ' => Some("po"),

        // M-line
        'ま' | 'マ' => Some("ma"),
        'み' | 'ミ' => Some("mi"),
        'む' | 'ム' => Some("mu"),
        'め' | 'メ' => Some("me"),
        'も' | 'モ' => Some("mo"),

        // Y-line
        'や' | 'ヤ' => Some("ya"),
        'ゆ' | 'ユ' => Some("yu"),
        'よ' | 'ヨ' => Some("yo"),

        // R-line
        'ら' | 'ラ' => Some("ra"),
        'り' | 'リ' => Some("ri"),
        'る' | 'ル' => Some("ru"),
        'れ' | 'レ' => Some("re"),
        'ろ' | 'ロ' => Some("ro"),

        // W-line
        'わ' | 'ワ' => Some("wa"),
        'を' | 'ヲ' => Some("wo"),

        // N / M (Hatsuon)
        'ん' | 'ン' => Some("n"),

        // Long vowel mark (Chouonpu: ー)
        'ー' => Some("-"),

        _ => None,
    }
}

// =========================================================================
// General Smart Romanizer (Korean + Japanese combined)
// =========================================================================

pub fn romanize_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Step 1: Korean romanization for any Hangul characters
    let korean_processed = romanize_korean(text);

    // Step 2: Japanese Romaji for Kanji, Hiragana, and Katakana via Kakasi
    // kakasi converts Kanji -> Romaji, Hiragana -> Romaji, Katakana -> Romaji
    let japanese_processed = kakasi::convert(&korean_processed).romaji;

    // Step 3: Pass through internal romanize_japanese as backup for any remaining kana nuances
    romanize_japanese(&japanese_processed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_script() {
        assert_eq!(ScriptDetector::detect_script("하츠투하츠"), ScriptType::Korean);
        assert_eq!(ScriptDetector::detect_script("ILLIT"), ScriptType::None);
        assert_eq!(ScriptDetector::detect_script("아일릿 (ILLIT)"), ScriptType::Korean);
        assert_eq!(ScriptDetector::detect_script("サクラ"), ScriptType::Japanese);
        assert_eq!(ScriptDetector::detect_script("ひらがな"), ScriptType::Japanese);
    }

    #[test]
    fn test_korean_romanization() {
        assert_eq!(romanize_korean("하츠투하츠"), "hacheutuhacheu");
        assert_eq!(romanize_korean("아일릿"), "aillit");
        assert_eq!(romanize_korean("안녕하세요"), "annyeonghaseyo");
        assert_eq!(romanize_korean("Hearts2Hearts (하츠투하츠)"), "Hearts2Hearts (hacheutuhacheu)");
    }

    #[test]
    fn test_japanese_romanization() {
        assert_eq!(romanize_japanese("さくら"), "sakura");
        assert_eq!(romanize_japanese("トーキョー"), "to-kyo-");
        assert_eq!(romanize_japanese("ちょっと"), "chotto");
        assert_eq!(romanize_japanese("きょう"), "kyo");
        assert_eq!(romanize_japanese("Toki Yo Tomare"), "Toki Yo Tomare");
    }

    #[test]
    fn test_general_romanize() {
        assert_eq!(romanize_text("아일릿"), "aillit");
        assert_eq!(romanize_text("さくら"), "sakura");
        assert_eq!(romanize_text("15-LOVE (하츠투하츠)"), "15-LOVE (hacheutuhacheu)");
        // Test Kanji from user's lyrics (甘い外側 苦い内側)
        let kanji_lyric = "甘い外側 苦い内側";
        let rom = romanize_text(kanji_lyric);
        assert!(!rom.contains("甘") && !rom.contains("外") && !rom.contains("苦") && !rom.contains("内"));
        println!("Kanji lyric rom: {}", rom);
    }
}
