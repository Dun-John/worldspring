//! The filter of sayable letter runs, and small helpers over finished names.

use super::*;

/// A word of the approved lists (any culture's), as it stands.
pub fn is_training(word: &str) -> bool {
    CULTURES.iter().any(|c| c.train.iter().any(|t| t.eq_ignore_ascii_case(word)))
}

pub(super) fn vowel_at(b: &[u8], i: usize) -> bool {
    let plain = |c: u8| matches!(c, b'a' | b'e' | b'i' | b'o' | b'u');
    match b[i] {
        c if plain(c) => true,
        // y is a vowel between consonants (or at an end beside one).
        b'y' => !(i > 0 && plain(b[i - 1])) && !(i + 1 < b.len() && plain(b[i + 1])),
        _ => false,
    }
}

/// How often a letter is drawn after a pair it followed in the lists: as often as it did, to
/// three times (so the lists' commonest runs don't come round in every word).
pub(super) fn weight(_: &[u8; 3], n: u32) -> u32 {
    n.min(3)
}

pub(super) fn letters(word: &str) -> Vec<u8> {
    word.bytes().filter(u8::is_ascii_alphabetic).map(|c| c.to_ascii_lowercase()).collect()
}

/// How many syllables (runs of vowels) a word has.
pub fn syllables(word: &str) -> usize {
    let b = letters(word);
    (0..b.len()).filter(|&i| vowel_at(&b, i) && (i == 0 || !vowel_at(&b, i - 1))).count()
}

/// What is wrong with one word of a name, if anything. `culture`: it is a culture word of that
/// culture (the strict rules: 10 letters and 3 syllables at most, only the allowed consonant
/// clusters, no y before two consonants, no doubled vowel, no syllable said twice running, no
/// comic bit of a real word, `q` only in the dry and steppe tongues, ...); otherwise a compound or
/// English word (14 letters, runs of up to 5 consonants: "Kingsbridge").
pub fn fault(word: &str, culture: Option<usize>) -> Option<&'static str> {
    let b = letters(word);
    if b.len() > if culture.is_some() { 10 } else { 14 } {
        return Some("too long");
    }
    // (English words have some: "Kettle", "Hoopoe", "Turquoise".)
    for p in b.windows(2) {
        if BANNED.iter().any(|x| x.as_bytes() == p && (culture.is_some() || p != b"tl")) {
            return Some("banned pair");
        }
    }
    if culture.is_some() && bad_run(&b) {
        return Some("bad letter run");
    }
    if b.starts_with(b"ts") || b.ends_with(b"eh") {
        return Some("ts- or -eh");
    }
    for (i, &c) in b.iter().enumerate() {
        if c == b'q' && !matches!(b.get(i + 1), Some(b'u' | b'a')) {
            return Some("q not before u or a");
        }
    }
    if culture.is_some_and(|c| c != 3 && c != 4) && b.contains(&b'q') {
        return Some("q outside the dry and steppe tongues");
    }
    if b.windows(3).any(|t| t[0] == t[1] && t[1] == t[2]) {
        return Some("tripled letter");
    }
    let (mut vs, mut cs) = (0, 0);
    for i in 0..b.len() {
        if vowel_at(&b, i) {
            (vs, cs) = (vs + 1, 0);
        } else {
            (vs, cs) = (0, cs + 1);
        }
        if vs >= 3 {
            return Some("three vowels");
        }
        match culture {
            Some(_) if cs == 3 && !CLUSTERS.iter().any(|x| x.as_bytes() == &b[i - 2..=i]) => return Some("consonant cluster"),
            Some(_) if cs >= 4 => return Some("four consonants"),
            None if cs >= 6 => return Some("six consonants"),
            _ => {}
        }
    }
    let Some(c) = culture else { return None };
    if syllables(word) > 3 {
        return Some("four syllables");
    }
    for i in 0..b.len() {
        if b[i] == b'y' && vowel_at(&b, i) && i + 2 < b.len() && !vowel_at(&b, i + 1) && !vowel_at(&b, i + 2) {
            return Some("y before two consonants");
        }
    }
    let plain = |x: u8| matches!(x, b'a' | b'e' | b'i' | b'o' | b'u');
    if b.windows(2).any(|p| p[0] == p[1] && plain(p[0]) && !(p[0] == b'a' && c == 4)) {
        return Some("doubled vowel");
    }
    if b.windows(4).any(|w| w[0] == w[2] && w[1] == w[3] && plain(w[0]) != plain(w[1])) {
        return Some("syllable said twice");
    }
    // w after a vowel only at the end ("Haddow", not "Cygeawfowm", "Tewom").
    for i in 1..b.len() {
        if b[i] == b'w' && plain(b[i - 1]) && i + 1 < b.len() {
            return Some("w after a vowel inside a word");
        }
    }
    if b.windows(2).any(|p| p == b"iu" || p == b"ui") || (c == 4 && b.ends_with(b"ng")) {
        return Some("iu or ui, or -ng in the steppe tongue");
    }
    // y after a vowel only at the end or before a vowel ("Hrossey", not "Loyw", "Beaym").
    for i in 1..b.len() {
        if b[i] == b'y' && plain(b[i - 1]) && i + 1 < b.len() && !plain(b[i + 1]) {
            return Some("y after a vowel before a consonant");
        }
        if b[i] == b'y' && b.get(i + 1) == Some(&b'i') {
            return Some("yi in a word");
        }
    }
    if c != 1 && b.windows(2).any(|p| p == b"ou") {
        return Some("ou outside the west tongue");
    }
    // Vowel pairs: only the culture's own ("ei", "au" in the north; "ia", "io" in the south),
    // one a word, and in a word of three syllables only in the last ("Valeria", not
    // "Siopenus", "Nugaent", "Muek").
    let pairs: &[&[u8]] = match c {
        0 => &[b"au", b"ei"],
        1 => &[b"ae", b"ea", b"ai"],
        2 => &[b"ia", b"io", b"au"],
        3 => &[b"ai"],
        _ => &[b"ai", b"ei", b"aa"],
    };
    let at: Vec<usize> = (0..b.len().saturating_sub(1)).filter(|&i| plain(b[i]) && plain(b[i + 1])).collect();
    if at.iter().any(|&i| !pairs.contains(&&b[i..i + 2])) || at.len() > 1 {
        return Some("a vowel pair not of the tongue");
    }
    if let Some(&i) = at.first() {
        if syllables(word) == 3 && b[i + 2..].iter().any(|&x| plain(x)) {
            return Some("a vowel pair early in a long word");
        }
        // ("Fusmia": ia and io after one consonant.)
        if c == 2 && i >= 2 && !plain(b[i - 1]) && !plain(b[i - 2]) {
            return Some("a vowel pair after two consonants");
        }
        // ("Brucugiar", "Bonruvial": the south's pairs end a word or come before a vowel.)
        if c == 2 && i + 3 == b.len() {
            return Some("a vowel pair before a last consonant");
        }
    }
    if let Some(f) = ban(word) {
        return Some(f);
    }
    // Endings of no tongue here ("Mijangek", "Yichik", "Tikege"; -ik is the north's, "Dravik").
    if b.ends_with(b"ek") || b.ends_with(b"ege") || (c != 0 && b.ends_with(b"ik")) {
        return Some("-ek, -ege or -ik outside its tongue");
    }
    // The steppe's long words do not end in two closed syllables running ("Chonsoknur").
    if c == 4 && syllables(word) >= 3 && !vowel_at(&b, b.len() - 1) {
        let last = (0..b.len()).rev().find(|&i| plain(b[i]));
        let before = last.and_then(|l| (0..l).rev().find(|&i| plain(b[i])));
        if let (Some(l), Some(v)) = (last, before) {
            if l - v > 2 {
                return Some("two closed syllables ending a steppe word");
            }
        }
    }
    if (c == 0 || c == 1) && b.ends_with(b"i") {
        return Some("-i in the north or west tongue");
    }
    if COMIC.iter().any(|x| b.windows(x.len()).any(|w| w == x.as_bytes())) || REAL_NAMES.iter().any(|x| x.as_bytes() == b) {
        return Some("a real word");
    }
    None
}

/// The later bans on a culture word (with an ending on or not): no Hr- or Bj-, Ll only before a
/// vowel, no -au, -dry, -ian nor hard final pair (-gt, -st), `q` only before a or i, no
/// `NEW_RUNS`, no English start or word, no two r-clusters a vowel apart ("Trobr-") nor three
/// r's, no first syllable said back ("Sist", "Rers"), no hiss twice ("Shalaimesh", "Jezzaz").
pub fn ban(word: &str) -> Option<&'static str> {
    let b = letters(word);
    let plain = |x: u8| matches!(x, b'a' | b'e' | b'i' | b'o' | b'u');
    let cons = |x: u8| !plain(x) && x != b'y';
    let stop = |x: u8| matches!(x, b'b' | b'c' | b'd' | b'f' | b'g' | b'k' | b'p' | b't');
    let n = b.len();
    if b.starts_with(b"hr") || b.starts_with(b"bj") {
        return Some("Hr- or Bj-");
    }
    // ("Brytathy", "Yukhinun", "Yichik": no y right after a starting cluster, nor yu-, yi-.)
    if (n >= 3 && cons(b[0]) && cons(b[1]) && b[2] == b'y') || b.starts_with(b"yu") || b.starts_with(b"yi") {
        return Some("a clumsy y at the start");
    }
    if b.starts_with(b"ll") && !b.get(2).is_some_and(|&x| plain(x)) {
        return Some("Ll before a consonant");
    }
    if b.ends_with(b"au") || b.ends_with(b"dry") || b.ends_with(b"try") || b.ends_with(b"ian") {
        return Some("a bad ending");
    }
    if n >= 3 && matches!(&b[n - 2..], b"gt" | b"kt" | b"pt" | b"bt" | b"dt" | b"zt" | b"st" | b"ht" | b"gd" | b"kd") {
        return Some("a hard final pair");
    }
    for i in 0..n {
        if b[i] == b'q' && !matches!(b.get(i + 1), Some(b'a' | b'i')) {
            return Some("q not before a or i");
        }
    }
    if NEW_RUNS.iter().any(|x| b.windows(x.len()).any(|w| w == x.as_bytes())) {
        return Some("bad letter run");
    }
    if JOKE_START.iter().any(|x| b.starts_with(x.as_bytes())) || SHORT_WORDS.split_whitespace().any(|x| x.as_bytes() == b) {
        return Some("reads as English");
    }
    if b.windows(5).any(|w| stop(w[0]) && w[1] == b'r' && plain(w[2]) && stop(w[3]) && w[4] == b'r') || b.iter().filter(|&&x| x == b'r').count() > 2 {
        return Some("r-clusters piled up");
    }
    if n >= 3 && cons(b[0]) && b[0] == b[2] && plain(b[1]) && b.get(3).is_none_or(|&x| cons(x)) {
        return Some("first syllable said back");
    }
    if b.windows(2).filter(|p| p == b"sh").count() >= 2 || b.iter().filter(|&&x| x == b'z').count() >= 2 {
        return Some("a hiss twice");
    }
    // ("Estedsted", "Ulfulf", "Gwenwen".)
    if (0..n.saturating_sub(5)).any(|i| b[i + 3..].windows(3).any(|w| w == &b[i..i + 3])) {
        return Some("a run said twice");
    }
    None
}

/// One letter put in, left out or changed in a training word ("Hadow", "Fiona").
pub(super) fn near_training(w: &str) -> bool {
    let a = letters(w);
    let near = |t: &[u8]| {
        if a.len().abs_diff(t.len()) > 1 {
            return false;
        }
        let (mut i, mut j, mut k) = (0usize, a.len(), t.len());
        while i < j.min(k) && a[i] == t[i] {
            i += 1;
        }
        while j > i && k > i && a[j - 1] == t[k - 1] {
            j -= 1;
            k -= 1;
        }
        j - i <= 1 && k - i <= 1
    };
    a.len() >= 4 && CULTURES.iter().any(|c| c.train.iter().any(|t| near(&letters(t))))
}

pub(super) fn bad_run(b: &[u8]) -> bool {
    BAD_RUNS.iter().any(|x| b.windows(x.len()).any(|w| w == x.as_bytes()))
}

/// What is wrong with a culture word with an ending on ("Haldurrud", "Grirddal"): a compound's
/// faults, a bad letter run or "tl", a doubled consonant in a run of three, more than three
/// syllables.
pub(super) fn place_fault(word: &str) -> Option<&'static str> {
    if let Some(f) = fault(word, None) {
        return Some(f);
    }
    let b = letters(word);
    if bad_run(&b) || b.windows(2).any(|p| p == b"tl") {
        return Some("bad letter run");
    }
    if let Some(f) = ban(word) {
        return Some(f);
    }
    // (A stem and its ending, put together, can end as no tongue here does: "Tikege".)
    if b.ends_with(b"ek") || b.ends_with(b"ege") {
        return Some("-ek or -ege");
    }
    if b.windows(3).any(|t| (t[0] == t[1] || t[1] == t[2]) && t.iter().all(|&x| !matches!(x, b'a' | b'e' | b'i' | b'o' | b'u' | b'y'))) {
        return Some("doubled consonant in a cluster");
    }
    if syllables(word) > 3 {
        return Some("four syllables");
    }
    None
}

/// The first fault in a whole name (each of its words as a compound), with the word.
pub fn name_fault(name: &str) -> Option<(String, &'static str)> {
    words(name).find_map(|w| fault(&w, None).map(|f| (w, f)))
}

/// A name's words, without possessives and punctuation ("St. Aldric's Cross": St, Aldric, Cross).
pub fn words(name: &str) -> impl Iterator<Item = String> + '_ {
    name.split([' ', '-']).map(|w| w.trim_end_matches("'s").trim_end_matches('\'').chars().filter(char::is_ascii_alphabetic).collect::<String>()).filter(|w| !w.is_empty())
}

/// `a` and `b` run together ("Raven" + "holm"), if they meet well: no banned pair or tripled
/// letter where they join, no three vowels, at most `run` consonants across the join.
pub(super) fn join(a: &str, b: &str, run: usize) -> Option<String> {
    let s = format!("{a}{}", b.to_lowercase());
    let bytes: Vec<u8> = s.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let k = a.len();
    if k == 0 || k >= bytes.len() {
        return Some(s);
    }
    if BANNED.iter().any(|x| x.as_bytes() == &bytes[k - 1..=k]) || bytes.windows(3).any(|t| t[0] == t[1] && t[1] == t[2]) {
        return None;
    }
    let v = |i: usize| vowel_at(&bytes, i);
    let kind = v(k);
    if v(k - 1) != kind {
        return Some(s);
    }
    let (mut lo, mut hi) = (k - 1, k + 1);
    while lo > 0 && v(lo - 1) == kind {
        lo -= 1;
    }
    while hi < bytes.len() && v(hi) == kind {
        hi += 1;
    }
    if hi - lo > if kind { 2 } else { run } { None } else { Some(s) }
}

/// `a` + `b` run together, or apart when they meet badly ("Ash Haven").
pub(super) fn compound(a: &str, b: &str, run: usize) -> String {
    // (Long ones apart too: "Whistling Fire", not "Whistlingfire".)
    join(a, b, run).filter(|_| a.len() + b.len() <= 12).unwrap_or_else(|| format!("{a} {}", capitalize(b)))
}

/// Whether a name is all English (each word an English place-name piece, or two run together:
/// "Ravenholm", "Kingsbridge"), not made with a culture word.
pub fn is_english(name: &str) -> bool {
    static PIECES: OnceLock<BTreeSet<String>> = OnceLock::new();
    let pieces = PIECES.get_or_init(|| {
        let mut p: BTreeSet<String> = BTreeSet::new();
        for c in &CULTURES {
            p.extend(c.pre.iter().chain(c.suf).chain(c.adj).map(|w| w.to_lowercase()));
        }
        let more = ADJ.iter().chain(&ROYAL).chain(&ROYAL_SUF).chain(&GRIM_PRE).chain(&GRIM_ADJ).chain(&["Great", "Little", "Upper", "Lower", "Nether", "Long", "East", "West"]);
        p.extend(more.map(|w| w.to_lowercase()));
        p
    });
    words(name).all(|w| {
        let w = w.to_lowercase();
        pieces.contains(&w) || (2..w.len().saturating_sub(1)).any(|k| pieces.contains(&w[..k]) && pieces.contains(&w[k..]))
    })
}

/// A name's head noun, what the place is called: "Camp" of "Hrolf's Camp", "Tower" of "The
/// Tower of Kald", "Sands" of "The Sands of Zahir".
pub fn site_noun(n: &str) -> &str {
    let head = n.split(" of ").next().unwrap_or(n);
    head.rsplit(' ').next().unwrap_or(head).trim_end_matches("'s")
}
