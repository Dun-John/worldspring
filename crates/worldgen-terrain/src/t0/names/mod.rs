//! Names. Regions of the map belong to a handful of invented "cultures" (cold north, temperate
//! west, warm south, dry lands, open steppe), so nearby places sound related. A culture word comes
//! either from a small letter model (order-2 Markov, learnt from a short approved list of words per
//! culture) or from a shape template on the culture's onsets, vowels and codas; every one passes a
//! filter of sayable letter runs (`fault`) and is never a word of the lists itself. Places mix
//! culture words with English in each culture's measure (the west mostly English compounds, the
//! others mostly their own words), with "X of Y" forms, saints and kings, and grim names mostly on
//! dangerous ground. Names are unique across the world.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::core::rng::Pcg32;

mod fault;
mod words;

pub use fault::*;
pub use words::*;

/// The kinds of place where grim names belong.
fn dangerous(kind: NameKind) -> bool {
    use NameKind::*;
    matches!(kind, Swamp | Blight | Ashlands | Ruin | Cave | Mine | LavaTube | Pass)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameKind {
    Continent,
    Ocean,
    Sea,
    Bay,
    Island,
    Range,
    Peak,
    Pass,
    Volcano,
    Lake,
    SaltLake,
    SaltFlat,
    River,
    Waterfall,
    Forest,
    Jungle,
    Taiga,
    Desert,
    Swamp,
    Plains,
    Tundra,
    Glacier,
    Blight,
    Ashlands,
    /// A region drawn and named in the sketch (always named, so rarely asked for).
    Region,
    Settlement,
    Ruin,
    Tower,
    Camp,
    Cave,
    Mine,
    LavaTube,
}

/// A culture's letter model: how often each letter (1–26; 0 the word's edge) follows each pair.
struct Markov {
    /// (a, b, c) sorted, each with how often c follows a, b.
    next: Vec<([u8; 3], u32)>,
}

impl Markov {
    fn learn(words: &[&str]) -> Self {
        let mut counts: BTreeMap<[u8; 3], u32> = BTreeMap::new();
        for w in words {
            let mut s = vec![0u8, 0];
            s.extend(w.bytes().filter(u8::is_ascii_alphabetic).map(|b| b.to_ascii_lowercase() - b'a' + 1));
            s.push(0);
            for t in s.windows(3) {
                *counts.entry([t[0], t[1], t[2]]).or_default() += 1;
            }
        }
        Self { next: counts.into_iter().collect() }
    }

    /// A word of at most `max` letters, lower case (None: the walk ran long).
    fn walk(&self, rng: &mut Pcg32, max: usize) -> Option<String> {
        let (mut a, mut b) = (0u8, 0u8);
        let mut s = String::new();
        loop {
            let lo = self.next.partition_point(|(t, _)| (t[0], t[1]) < (a, b));
            let hi = self.next.partition_point(|(t, _)| (t[0], t[1]) <= (a, b));
            let total: u32 = self.next[lo..hi].iter().map(|(t, n)| weight(t, *n)).sum();
            if total == 0 {
                return None;
            }
            let mut r = rng.below(total);
            let mut c = 0;
            for (t, n) in &self.next[lo..hi] {
                let n = weight(t, *n);
                if r < n {
                    c = t[2];
                    break;
                }
                r -= n;
            }
            if c == 0 {
                return Some(s);
            }
            s.push((b'a' + c - 1) as char);
            if s.len() > max {
                return None;
            }
            (a, b) = (b, c);
        }
    }
}

fn markov(culture: usize) -> &'static Markov {
    static MODELS: OnceLock<Vec<Markov>> = OnceLock::new();
    &MODELS.get_or_init(|| CULTURES.iter().map(|c| Markov::learn(c.train)).collect())[culture]
}

/// How many features of `kind` a world names with head noun `h`: the rare ones twice, a few
/// overused ones a little more, any other a site's six times (land's as often as it comes).
fn noun_cap(kind: NameKind, h: &str) -> Option<u32> {
    use NameKind::*;
    const CAPPED: [(&str, u32); 9] = [("Cauldron", 2), ("Wilds", 2), ("Deep", 2), ("Ocean", 3), ("Sands", 3), ("Halls", 4), ("Camp", 5), ("Tower", 5), ("Mine", 5)];
    if RARE_NOUNS.contains(&h) {
        return Some(2);
    }
    if let Some(&(_, k)) = CAPPED.iter().find(|x| x.0 == h) {
        return Some(k);
    }
    matches!(kind, Ruin | Tower | Camp | Cave | Mine | LavaTube).then_some(6)
}

pub struct Namer {
    rng: Pcg32,
    /// Settlements' and sites' own seed (when they have one): each draws from a stream of its
    /// own, keyed by where it stands, so neither the land's names (more or fewer regions, say)
    /// nor a settlement or site added or taken away elsewhere change theirs (unless the one
    /// drawn first comes out too like a name given before: see `fresh`).
    sites: Option<u64>,
    used: BTreeSet<String>,
    /// How often each word (lower case) has gone into a name given out, and each culture word's
    /// first four letters; the culture words given out; and settlements named per culture, and
    /// per culture and last two letters: so no adjective, stem or ending runs through a world.
    seen: BTreeMap<String, u32>,
    stems: BTreeMap<String, u32>,
    stems3: BTreeMap<String, u32>,
    own: BTreeSet<String>,
    settled: [u32; 5],
    ends: BTreeMap<(usize, [u8; 2]), u32>,
    /// Per culture, how often each of its endings ("-heim") has gone on a settlement; and how
    /// often each head noun (`site_noun`: "Camp", "Halls") has named a feature.
    endings: BTreeMap<(usize, &'static str), u32>,
    heads: BTreeMap<String, u32>,
    /// The culture words drawn for the name being made, and the ending put on one.
    drawn: Vec<String>,
    ending: Option<&'static str>,
    /// Culture words made, and how many of them came from the letter model.
    pub made: u32,
    pub from_model: u32,
    /// When set, the words of each name given out made from culture words (namecheck's).
    pub log: Option<Vec<String>>,
    /// The underground theme (`under::THEMES` key) of the next site named, when known: its
    /// words ("Rime", "Glimmer") go into the name.
    pub theme: Option<&'static str>,
}

impl Namer {
    pub fn new(seed: u64) -> Self {
        Self::make(seed, None)
    }

    /// A namer whose settlements and sites draw from streams of their own (`site_seed` and where
    /// they stand: `name_at`).
    pub fn with_sites(seed: u64, site_seed: u64) -> Self {
        Self::make(seed, Some(site_seed))
    }

    fn make(seed: u64, sites: Option<u64>) -> Self {
        Self {
            rng: Pcg32::new(seed, 11),
            sites,
            used: BTreeSet::new(),
            seen: BTreeMap::new(),
            stems: BTreeMap::new(),
            stems3: BTreeMap::new(),
            own: BTreeSet::new(),
            settled: [0; 5],
            ends: BTreeMap::new(),
            endings: BTreeMap::new(),
            heads: BTreeMap::new(),
            drawn: Vec::new(),
            ending: None,
            made: 0,
            from_model: 0,
            log: None,
            theme: None,
        }
    }

    pub fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.rng.below(xs.len() as u32) as usize]
    }

    pub fn below(&mut self, n: u32) -> u32 {
        self.rng.below(n)
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.rng.next_f64() < p
    }

    /// A shape-template word of `syl` syllables, lower case.
    fn template(&mut self, c: &Culture, syl: u32) -> String {
        let mut s = String::new();
        for k in 0..syl {
            if k == 0 || self.rng.below(3) > 0 {
                s.push_str(self.pick(c.onsets));
            }
            s.push_str(self.pick(c.vowels));
            if k + 1 == syl || self.rng.below(3) == 0 {
                s.push_str(self.pick(c.codas));
            }
        }
        s
    }

    /// Two of the culture's curated roots run together ("Thor" + "geir"), lower case.
    fn rooted(&mut self, c: &Culture) -> Option<String> {
        let (a, b) = (self.pick(c.roots.0), self.pick(c.roots.1));
        let b = if a.ends_with(['a', 'e', 'i', 'o', 'u']) && b.starts_with(['a', 'e', 'i', 'o', 'u']) { &b[1..] } else { b };
        join(a, b, 3).map(|w| w.to_lowercase())
    }

    /// A culture word of 1–3 syllables (by length: about 3–6, 4–8, 6–10 letters), capitalized:
    /// from the letter model (two in five), a template or two curated roots, sayable
    /// (`fault`), never a training word nor a letter off one, a model's word not starting as
    /// one does (its first four letters), and only now and then Bj-, Fj- or Hj-.
    pub fn word(&mut self, culture: usize, min_syl: u32, max_syl: u32) -> String {
        let ci = culture % CULTURES.len();
        let c = &CULTURES[ci];
        let lo = [3, 4, 6][(min_syl.clamp(1, 3) - 1) as usize];
        let hi = [6, 8, 10][(max_syl.clamp(1, 3) - 1) as usize];
        let like_training = |w: &str| w.len() >= 4 && CULTURES.iter().any(|c| c.train.iter().any(|t| t.len() >= 4 && t[..4].eq_ignore_ascii_case(&w[..4])));
        for _ in 0..40 {
            let r = self.rng.below(20);
            let w = if r < 13 {
                match markov(ci).walk(&mut self.rng, hi) {
                    Some(w) if !like_training(&w) => w,
                    _ => continue,
                }
            } else if r < 17 || max_syl < 2 {
                let syl = min_syl + self.rng.below(max_syl.max(min_syl) - min_syl + 1);
                self.template(c, syl)
            } else {
                match self.rooted(c) {
                    Some(w) => w,
                    None => continue,
                }
            };
            if (w.starts_with("bj") || w.starts_with("fj") || w.starts_with("hj")) && self.rng.below(10) > 0 {
                continue;
            }
            if (lo..=hi).contains(&w.len()) && fault(&w, Some(ci)).is_none() && !is_training(&w) && !near_training(&w) {
                if self.drawn.len() >= 64 {
                    self.drawn.clear();
                }
                self.drawn.push(w.clone());
                self.made += 1;
                self.from_model += (r < 13) as u32;
                return capitalize(&w);
            }
        }
        // Consonant, vowel by turns: always sayable.
        let cons: Vec<&str> = c.onsets.iter().copied().filter(|o| o.len() == 1 && *o != "q" && *o != "y").collect();
        let vow: Vec<&str> = c.vowels.iter().copied().filter(|v| v.len() == 1 && *v != "y").collect();
        let mut s = String::new();
        for k in 0..lo.max(4) {
            s.push_str(if k % 2 == 0 { self.pick(&cons) } else { self.pick(&vow) });
        }
        self.drawn.push(s.clone());
        self.made += 1;
        capitalize(&s)
    }

    /// A person's name in a culture ("Hrolf", "Maddo"): a short culture word.
    pub fn person(&mut self, culture: usize) -> String {
        let mut p = self.word(culture, 1, 2);
        for _ in 0..8 {
            if p.len() >= 4 {
                break;
            }
            p = self.word(culture, 1, 2);
        }
        p
    }

    /// Unique name for a feature of `kind` in the given culture region.
    pub fn name(&mut self, kind: NameKind, culture: usize) -> String {
        self.unique(kind, culture)
    }

    /// `name` for a feature standing at `at` (ft): a settlement or site draws from its own
    /// stream there, when this namer gives them one (`with_sites`).
    pub fn name_at(&mut self, kind: NameKind, culture: usize, at: [i64; 2]) -> String {
        use NameKind::*;
        let site = matches!(kind, Settlement | Ruin | Tower | Camp | Cave | Mine | LavaTube);
        if site && let Some(seed) = self.sites {
            let mut own = Pcg32::new(crate::core::rng::hash2(seed, at[0], at[1]), 13);
            std::mem::swap(&mut self.rng, &mut own);
            let n = self.unique(kind, culture);
            std::mem::swap(&mut self.rng, &mut own);
            return n;
        }
        self.unique(kind, culture)
    }

    fn unique(&mut self, kind: NameKind, culture: usize) -> String {
        let culture = culture % CULTURES.len();
        let ok = |n: &str| name_fault(n).is_none() && !words(n).any(|w| is_training(&w));
        for i in 0..60 {
            self.drawn.clear();
            self.ending = None;
            let n = self.candidate(kind, culture);
            if ok(&n) && (i >= 40 || self.fresh(&n, kind, culture)) && !self.used.contains(&n) {
                self.note(&n, kind, culture);
                return n;
            }
        }
        // Practically unreachable; lengthen the name until unique.
        for k in 0.. {
            self.drawn.clear();
            self.ending = None;
            let n = format!("{} {}", self.candidate(kind, culture), self.word(culture, 1, 1));
            if (k > 200 || ok(&n)) && !self.used.contains(&n) {
                self.note(&n, kind, culture);
                return n;
            }
        }
        unreachable!()
    }

    /// The culture words drawn for name `n` that are in it, with their first four letters.
    fn own_words(&self, n: &str) -> Vec<(String, String)> {
        let low = n.to_lowercase();
        let stem = |w: &str| w[..w.len().min(4)].to_string();
        self.drawn.iter().filter(|w| low.contains(&stem(w))).map(|w| (w.clone(), stem(w))).collect()
    }

    /// Name `n` is not too like the names given before: none of its culture words given out
    /// before, nor its stem (first four letters) twice or its first three letters three times;
    /// a head noun (`site_noun`) no more often than `noun_cap` says; a settlement's ending
    /// ("-heim") on at most ~10% of its culture's, its last two letters on ~15%.
    fn fresh(&self, n: &str, kind: NameKind, c: usize) -> bool {
        let stem3 = |w: &str| w[..w.len().min(3)].to_string();
        if self.own_words(n).iter().any(|(w, s)| self.own.contains(w) || self.stems.get(s).is_some_and(|&k| k >= 2) || (w.len() >= 5 && self.stems3.get(&stem3(w)).is_some_and(|&k| k >= 3))) {
            return false;
        }
        // ("The Wild Wilds".)
        let ws: Vec<String> = words(n).map(|w| w.to_lowercase()).filter(|w| w.len() >= 4).collect();
        if ws.iter().enumerate().any(|(i, a)| ws[i + 1..].iter().any(|b| a[..4] == b[..4])) {
            return false;
        }
        if kind != NameKind::Settlement {
            let h = site_noun(n);
            if noun_cap(kind, h).is_some_and(|cap| self.heads.get(h).is_some_and(|&k| k >= cap)) {
                return false;
            }
        }
        if kind == NameKind::Settlement {
            let share = |k: u32, p: f64| k >= 2 && (k + 1) as f64 > p * (self.settled[c] + 1) as f64;
            if share(self.ends.get(&(c, ending(n))).copied().unwrap_or(0), 0.15) || self.ending.is_some_and(|e| share(self.endings.get(&(c, e)).copied().unwrap_or(0), 0.1)) {
                return false;
            }
        }
        true
    }

    fn note(&mut self, n: &str, kind: NameKind, c: usize) {
        if let Some(log) = &mut self.log {
            let stems: Vec<String> = self.drawn.iter().map(|w| w[..w.len().min(4)].to_string()).collect();
            log.extend(words(n).filter(|w| stems.iter().any(|s| w.to_lowercase().contains(s.as_str()))));
        }
        for (w, s) in self.own_words(n) {
            *self.stems.entry(s).or_default() += 1;
            *self.stems3.entry(w[..w.len().min(3)].to_string()).or_default() += 1;
            self.own.insert(w);
        }
        if kind != NameKind::Settlement {
            *self.heads.entry(site_noun(n).to_string()).or_default() += 1;
        }
        for w in words(n) {
            *self.seen.entry(w.to_lowercase()).or_default() += 1;
        }
        if kind == NameKind::Settlement {
            self.settled[c] += 1;
            *self.ends.entry((c, ending(n))).or_default() += 1;
            if let Some(e) = self.ending {
                *self.endings.entry((c, e)).or_default() += 1;
            }
        }
        self.used.insert(n.to_string());
    }

    /// A describing word, the culture's own or a shared one, used at most twice in a world when
    /// it can be.
    fn adj(&mut self, c: usize) -> &'static str {
        let mut a = "";
        for _ in 0..6 {
            a = if self.rng.below(2) == 0 { self.pick(CULTURES[c].adj) } else { self.pick(&ADJ) };
            if self.seen.get(&a.to_lowercase()).is_none_or(|&k| k < 2) {
                break;
            }
        }
        a
    }

    /// One of `xs`, used at most twice in a world when it can be.
    fn rare_pick(&mut self, xs: &[&'static str]) -> &'static str {
        let mut a = "";
        for _ in 0..6 {
            a = self.pick(xs);
            if self.seen.get(&a.to_lowercase()).is_none_or(|&k| k < 2) {
                break;
            }
        }
        a
    }

    fn grim(&mut self, nouns: &[&str]) -> String {
        let n = self.pick(nouns);
        match self.rng.below(3) {
            0 => format!("The {} {n}", self.pick(&GRIM_ADJ)),
            1 => format!("{} {n}", self.pick(&GRIM_POSS)),
            _ => format!("The {n} of {}", self.pick(&GRIM_OF)),
        }
    }

    fn themed(&mut self, theme: &str, plain: &str) -> Option<String> {
        let (_, adjs, nouns) = THEME_WORDS.iter().find(|t| t.0 == theme)?;
        let a = self.pick(adjs);
        let n = if self.rng.below(3) == 0 { plain } else { self.pick(nouns) };
        // ("Glimmer Grotto", "Anvil Halls"; but "The Forgotten Dungeon", "The Sunken Temple".)
        let bare = !a.ends_with('\'') && !a.ends_with('s') && !a.ends_with("ed") && !a.ends_with("en") && !a.ends_with("ing");
        Some(if bare && self.rng.below(4) == 0 { format!("{a} {n}") } else { format!("The {a} {n}") })
    }

    /// A culture word with one of the culture's endings on, sayable as one (`place_fault`);
    /// else a bare culture word.
    fn with_ending(&mut self, c: usize) -> String {
        for _ in 0..8 {
            let end = self.pick(CULTURES[c].endings);
            let stem = self.word(c, 1, if end.len() >= 4 { 1 } else { 2 });
            // A stem in -ur takes no ending ("Valdurvik", "Haldurrud").
            if stem.ends_with("ur") {
                continue;
            }
            // A vowel meeting a vowel: the stem's gives way ("Corva" + "ona": "Corvona").
            let stem = if end.starts_with(['a', 'e', 'i', 'o', 'u']) { stem.trim_end_matches(['a', 'e', 'i', 'o', 'u', 'y']).to_string() } else { stem };
            if let Some(w) = join(&stem, end, 3)
                && place_fault(&w).is_none()
            {
                self.ending = Some(end);
                return w;
            }
        }
        self.word(c, 2, 3)
    }

    fn settlement(&mut self, c: usize) -> String {
        let cu = &CULTURES[c];
        let r = self.rng.next_f64();
        // (Grim compounds are English: the west's and now and then the north's.)
        if r < [0.01, 0.03, 0.0, 0.0, 0.0][c] {
            let pre = self.pick(&GRIM_PRE);
            let suf = self.pick(cu.suf);
            return compound(pre, suf, 4);
        }
        if r < 0.08 {
            let p = self.person(c);
            return match (c, self.rng.below(3)) {
                (0, 0 | 1) | (1, 0) => format!("St. {p}'s"),
                (0 | 1, _) => {
                    let (a, b) = (self.pick(&ROYAL), self.pick(&ROYAL_SUF));
                    format!("{a}{b}")
                }
                (2, 0 | 1) => format!("{} {p}", if p.ends_with('a') { "Santa" } else { "San" }),
                (2, _) => format!("Porto {p}"),
                (3, 0) => format!("Bir {p}"),
                (3, 1) => format!("Qasr {p}"),
                (3, _) => format!("Ain {p}"),
                (_, 0) => format!("{p}'s Cairn"),
                (_, _) => format!("{p}'s Ring"),
            };
        }
        if self.chance(cu.english) {
            let (a, b) = (self.pick(cu.pre), self.pick(cu.suf));
            let n = compound(a, b, 4);
            if c == 1 && self.rng.below(20) == 0 {
                return format!("{} {n}", self.pick(&["Great", "Little", "Upper", "Lower", "Nether", "Long", "East", "West"]));
            }
            return n;
        }
        if c == 2 {
            match self.rng.below(20) {
                0 | 1 => {
                    // A one-syllable stem of plain vowels: "Corvamare", not "Ciasamare".
                    for _ in 0..6 {
                        let stem = self.word(c, 1, 1);
                        let stem = stem.trim_end_matches(['a', 'e', 'i', 'o', 'u']);
                        if syllables(stem) == 1 && !stem.contains("ia") && !stem.contains("io") && ban(&format!("{stem}amare")).is_none() {
                            return compound(stem, "amare", 3);
                        }
                    }
                }
                2 => return format!("Monte {}", self.word(c, 2, 3)),
                3 => return format!("Castel {}", self.word(c, 2, 3)),
                4 => return format!("Porto {}", self.word(c, 2, 3)),
                _ => {}
            }
        }
        let bare = if c >= 2 { 45 } else { 25 };
        if self.rng.below(100) < bare {
            // (Five letters or more: the short ones come round in every world: "Pero", "Cano".)
            let mut w = self.word(c, 2, 3);
            for _ in 0..4 {
                if w.len() >= 5 {
                    break;
                }
                w = self.word(c, 2, 3);
            }
            return w;
        }
        self.with_ending(c)
    }

    fn candidate(&mut self, kind: NameKind, c: usize) -> String {
        use NameKind::*;
        if kind == Settlement {
            return self.settlement(c);
        }
        let eng = self.chance(CULTURES[c].nature);
        let grim = dangerous(kind) && self.chance(match kind {
            Blight | Swamp => 0.4,
            Ruin | Pass | Ashlands => 0.25,
            _ => 0.15,
        });
        let of_y = self.chance(0.08);
        let r = self.rng.below(4);
        // A site with a known theme takes its words, but for a grim name (or half the time in a
        // plain cave): a crystal cave is never "the Sun Hollow".
        if let (Some(t), Cave | Mine | LavaTube | Ruin) = (self.theme, kind)
            && !grim
            && (t != "cave" || self.rng.below(2) == 0)
        {
            let plain = match kind {
                Cave => self.pick(&CAVE_NOUNS),
                Mine => self.pick(&["Mine", "Shaft", "Pits", "Workings", "Delvings", "Adit"]),
                LavaTube => self.pick(&TUBE_NOUNS),
                _ => self.pick(RUIN_NOUNS[c]),
            };
            if let Some(n) = self.themed(t, plain) {
                return n;
            }
        }
        if grim {
            let nouns: &[&str] = match kind {
                Swamp => &["Mire", "Fen", "Marsh", "Bog", "Slough"],
                Blight => &["Wood", "Weald", "Waste", "Holt"],
                Ashlands => &["Cinders", "Ashes", "Barrens"],
                Ruin => &["Ruins", "Halls", "Keep", "Stones"],
                Cave => &["Caves", "Hollow", "Pit", "Grotto"],
                Mine => &["Mine", "Delve", "Pit"],
                LavaTube => &["Tubes", "Burrows", "Vents"],
                _ => &["Pass", "Gap", "Notch"],
            };
            return self.grim(nouns);
        }
        if of_y {
            let x = match kind {
                Ocean => Some("The Ocean"),
                Sea => Some("The Sea"),
                Bay => Some("The Bay"),
                Island => Some("The Isle"),
                Range => Some("The Mountains"),
                Lake => Some("The Lake"),
                Forest => Some("The Wood"),
                Plains => Some("The Vale"),
                Desert => Some("The Sands"),
                Tundra => Some("The Barrens"),
                Region => Some("The Land"),
                _ => None,
            };
            if let Some(x) = x {
                return format!("{x} of {}", self.pick(&OF_Y));
            }
        }
        let w = self.word(c, 2, 3);
        let short = self.word(c, 1, 2);
        let adj = self.adj(c);
        // Words for land the culture may not know by its name: a taiga, a jungle or a glacier
        // far from where the word belongs.
        let pines = if c == 0 { "Taiga" } else { ["Pinewood", "Pines", "Firwood"][self.rng.below(3) as usize] };
        let jungle = if matches!(c, 2 | 3) { "Jungle" } else { ["Wilds", "Greenwood", "Thickets"][self.rng.below(3) as usize] };
        let ice = if matches!(c, 0 | 4) { "Glacier" } else { ["Icefield", "Snowfield"][self.rng.below(2) as usize] };
        let plains = match c {
            0 => ["Heath", "Moors", "Plains"][self.rng.below(3) as usize],
            1 => ["Downs", "Vale", "Lea", "Plains"][self.rng.below(4) as usize],
            2 => ["Fields", "Meadows", "Plains"][self.rng.below(3) as usize],
            3 => ["Plain", "Flats", "Reaches"][self.rng.below(3) as usize],
            _ => ["Steppe", "Grasslands", "Plains"][self.rng.below(3) as usize],
        };
        let vast = self.rare_pick(&VAST);
        let desc = self.rare_pick(&CAVE_DESC);
        let one = self.word(c, 1, 1);
        // English run together ("Grasswater", "Ironspine") is the west's and the north's; the
        // others say it in two words.
        let runs = c <= 1;
        let ruin = self.pick(RUIN_NOUNS[c]);
        let tower = self.pick(TOWER_NOUNS[c]);
        let camp = self.pick(CAMP_NOUNS[c]);
        let fume = self.rare_pick(&FUME);
        let cone = ["Mountain", "Cone", "Crown", "Throne"][self.rng.below(4) as usize];
        let wood = match c {
            3 => ["Grove", "Palms", "Thickets"][self.rng.below(3) as usize],
            2 => ["Wood", "Grove", "Groves"][self.rng.below(3) as usize],
            _ => "Wood",
        };
        match (kind, eng, r) {
            (Continent, _, _) => w,
            (Ocean, false, _) => format!("The {w} Ocean"),
            (Ocean, true, 0) => format!("The {adj} Ocean"),
            (Ocean, true, 1) => format!("The {vast} Ocean"),
            (Ocean, true, 2) => format!("The {vast} Sea"),
            (Ocean, true, _) => format!("The {vast} Deep"),
            (Sea, false, _) => format!("The {w} Sea"),
            (Sea, true, 0 | 1) => format!("The {adj} Sea"),
            (Sea, true, _) => format!("The {vast} Sea"),
            (Bay, false, 0) => format!("Gulf of {w}"),
            (Bay, false, 1) => format!("Bay of {w}"),
            (Bay, false, _) => format!("{w} Bay"),
            (Bay, true, 0) => format!("{adj} Cove"),
            (Bay, true, _) => format!("The {adj} Bay"),
            (Island, false, 0 | 1) => format!("Isle of {w}"),
            (Island, false, _) => w,
            (Island, true, 0) if c == 0 => compound(adj, "holm", 4),
            (Island, true, _) => format!("{adj} Isle"),
            (Range, false, 0 | 1) => format!("The {w} Mountains"),
            (Range, false, _) => format!("The {w} Range"),
            (Range, true, 0 | 1) if runs => {
                let n = self.pick(&RANGE_NOUN);
                format!("The {} Mountains", compound(adj, n, 4))
            }
            (Range, true, 2) => format!("The {adj} Peaks"),
            (Range, true, _) => format!("The {adj} Mountains"),
            (Peak, false, 0 | 1) => format!("Mount {w}"),
            (Peak, false, _) => format!("{w} Peak"),
            (Peak, true, 0) => format!("{adj} Peak"),
            (Peak, true, 1) => format!("The {adj} Tooth"),
            (Peak, true, _) => format!("The {adj} Horn"),
            (Pass, false, 0 | 1) => format!("{short} Pass"),
            (Pass, false, _) => format!("The Pass of {short}"),
            (Pass, true, 0) => format!("{adj} Pass"),
            (Pass, true, _) => format!("The {adj} Gap"),
            (Volcano, false, _) => format!("Mount {w}"),
            (Volcano, true, 0) => format!("The {adj} Cauldron"),
            (Volcano, true, 1) if runs => compound(adj, "fire", 4),
            (Volcano, true, 1) => format!("Mount {w}"),
            (Volcano, true, _) => format!("The {fume} {cone}"),
            (Lake, false, 0 | 1) => format!("Lake {w}"),
            (Lake, false, _) => format!("{w} Mere"),
            (Lake, true, 0) => format!("{adj} Lake"),
            (Lake, true, 1) => format!("The {adj} Mere"),
            (Lake, true, _) if c == 1 => compound(adj, "water", 4),
            (Lake, true, _) => format!("The {adj} Lake"),
            (SaltLake, false, _) => format!("The {w} Salt Lake"),
            (SaltLake, true, _) => format!("The {adj} Salt Lake"),
            (SaltFlat, false, _) => format!("The {w} Salt Flats"),
            (SaltFlat, true, _) => format!("The {adj} Flats"),
            (River, false, 0 | 1) => format!("River {w}"),
            (River, false, _) => format!("The {w}"),
            (River, true, 0) => format!("The {adj} River"),
            (River, true, 1) if c == 0 => compound(adj, "beck", 4),
            (River, true, _) if c == 1 => compound(adj, "water", 4),
            (River, true, _) => format!("The {adj} River"),
            (Waterfall, false, _) => format!("{short} Falls"),
            (Waterfall, true, 0) if c == 0 => compound(adj, "force", 4),
            (Waterfall, true, _) => format!("The {adj} Falls"),
            (Forest, false, 0 | 1) => format!("{w} Forest"),
            (Forest, false, _) => format!("The Forest of {w}"),
            (Forest, true, 0) if runs => format!("The {}", compound(adj, "wood", 4)),
            (Forest, true, 1) if c == 1 => format!("The {adj} Weald"),
            (Forest, true, _) => format!("The {adj} {wood}"),
            (Jungle, false, _) => format!("The {w} {jungle}"),
            (Jungle, true, 0) => format!("The {adj} Wilds"),
            (Jungle, true, _) => format!("The {adj} {jungle}"),
            (Taiga, false, 0 | 1) => format!("The {w} Pinewood"),
            (Taiga, false, _) => format!("The {w} {pines}"),
            (Taiga, true, 0 | 1) => format!("The {adj} {pines}"),
            (Taiga, true, _) => format!("The {adj} Pines"),
            (Desert, false, 0 | 1) => format!("The {w} Desert"),
            (Desert, false, _) => format!("The Sands of {w}"),
            (Desert, true, 0 | 1) => format!("The {adj} Sands"),
            (Desert, true, _) => format!("The {adj} Waste"),
            (Swamp, false, 0 | 1) => format!("The {w} Marsh"),
            (Swamp, false, _) => format!("{w} Bog"),
            (Swamp, true, 0 | 1) if runs => format!("The {}", compound(adj, "fen", 4)),
            (Swamp, true, _) => format!("The {adj} Marsh"),
            (Plains, false, _) => format!("The {w} {plains}"),
            (Plains, true, _) => format!("The {adj} {plains}"),
            (Tundra, false, _) => format!("The {w} Barrens"),
            (Tundra, true, _) => format!("The {adj} Barrens"),
            (Glacier, false, _) => format!("The {w} {ice}"),
            (Glacier, true, 0 | 1) => format!("The {adj} {ice}"),
            (Glacier, true, _) => format!("The {adj} Icefield"),
            (Blight, _, 0) => format!("The Blighted Wood of {w}"),
            (Blight, _, 1) => format!("The {adj} Blight"),
            (Blight, _, _) => format!("The Withered {}", ["Wood", "Weald", "Grove", "Holt"][self.rng.below(4) as usize]),
            (Ashlands, false, _) => format!("The {w} Ashlands"),
            (Ashlands, true, _) => format!("The {adj} Cinders"),
            (Region, _, 0 | 1) => format!("The {w} Lands"),
            (Region, _, _) => w,
            (Settlement, _, _) => unreachable!(),
            (Ruin, _, 0) => format!("The Ruins of {w}"),
            (Ruin, _, 1) => format!("Old {w}"),
            (Ruin, _, 2) if c == 3 => format!("{ruin} {w}"),
            (Ruin, _, 2) => format!("{w} {ruin}"),
            (Ruin, _, _) => format!("The Fallen {ruin}"),
            (Tower, _, 0) => format!("{short}'s {tower}"),
            (Tower, _, 1) if self.rng.below(2) == 0 => format!("The {tower} of {short}"),
            (Tower, _, 1 | 2) => format!("The {adj} Spire"),
            (Tower, _, _) => format!("The {adj} {tower}"),
            (Camp, _, 0) => format!("{short}'s {camp}"),
            (Camp, _, 1) => format!("The {adj} {camp}"),
            (Camp, _, 2) => format!("{short} {camp}"),
            (Camp, _, _) => format!("The {adj} Stockade"),
            (Cave, _, 0) => format!("The {short} Caves"),
            (Cave, _, 1) => format!("The Caves of {one}"),
            (Cave, _, _) => format!("The {desc} {}", self.pick(&CAVE_NOUNS)),
            (Mine, _, 0 | 1) => format!("{short} Mine"),
            (Mine, _, 2) => format!("{short}'s Diggings"),
            (Mine, _, _) => format!("The {adj} Delve"),
            (LavaTube, _, 0) => format!("The {short} Tubes"),
            (LavaTube, _, 1) => format!("The {short} {}", self.pick(&TUBE_NOUNS)),
            (LavaTube, _, _) => format!("The {adj} Lava Tubes"),
        }
    }
}

/// A name's last two letters (its ending's family: "-an", "-ai").
fn ending(n: &str) -> [u8; 2] {
    let b = letters(n);
    match b.len() {
        0 => [0, 0],
        1 => [0, b[0]],
        k => [b[k - 2], b[k - 1]],
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
