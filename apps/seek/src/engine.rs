//! Local search over tool contracts (level 1 of Kortex Seek): no network, no AI, instant and
//! identical offline. Pure Rust: the host builds [`Doc`]s from the registry and asks for hits.
use std::collections::BTreeMap;

/// One searchable page: a tool, or one of its SEO variants (a preset such as `png-to-jpg`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Doc {
    pub id: String,
    pub variant: Option<String>,
    pub universe: String,
    pub names: BTreeMap<String, String>,
    /// For a variant: the names of its tool, to tell what the variant adds.
    pub tool_names: BTreeMap<String, String>,
    pub keywords: BTreeMap<String, Vec<String>>,
    pub intents: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub id: String,
    pub variant: Option<String>,
    pub score: u32,
}

/// Lower-case, accents removed, punctuation turned into spaces.
#[must_use]
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.to_lowercase().chars() {
        match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => out.push('a'),
            'ç' => out.push('c'),
            'è' | 'é' | 'ê' | 'ë' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' => out.push('i'),
            'ñ' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => out.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => out.push('u'),
            'ý' | 'ÿ' => out.push('y'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            'ß' => out.push_str("ss"),
            c if c.is_alphanumeric() => out.push(c),
            _ => out.push(' '),
        }
    }
    out
}

const STOPWORDS: &[&str] = &[
    // French
    "le", "la", "les", "un", "une", "des", "de", "du", "d", "l", "et", "ou", "en", "au", "aux",
    "pour", "par", "sur", "dans", "avec", "mon", "ma", "mes", "ce", "cet", "cette", "ces", "je",
    "veux", "voudrais", "faire", "comment", "puis", "peux", "qui", "que", "se", "son", "sa", "ses",
    // English
    "the", "a", "an", "of", "to", "for", "my", "in", "on", "and", "or", "with", "i", "want", "how",
    "can", "do", "is", "it", "from", "into", "some", "me",
];

/// Words that mean the same thing for a user, across FR/EN and spellings (folded).
const SYNONYMS: &[&[&str]] = &[
    &[
        "image", "images", "photo", "photos", "picture", "pictures", "img",
    ],
    &["password", "passwords", "passe", "mdp", "motdepasse"],
    &[
        "convert",
        "convertir",
        "converter",
        "convertisseur",
        "transformer",
        "changer",
    ],
    &[
        "calculate",
        "calculer",
        "calcul",
        "calculateur",
        "calculator",
        "compute",
    ],
    &["salary", "salaire", "paie", "payslip", "wage"],
    &[
        "merge",
        "fusionner",
        "combine",
        "combiner",
        "assembler",
        "joindre",
        "join",
    ],
    &["percent", "percentage", "pourcentage", "pct"],
    &[
        "loan",
        "pret",
        "credit",
        "emprunt",
        "mortgage",
        "immobilier",
    ],
    &["vat", "tva", "taxe"],
    &["tip", "pourboire", "gratuity"],
    &["color", "colour", "couleur"],
    &["contrast", "contraste"],
    &["text", "texte"],
    &["random", "aleatoire", "hasard"],
    &[
        "duplicate",
        "duplicates",
        "doublon",
        "doublons",
        "dedupliquer",
        "dedupe",
        "deduplicate",
    ],
    &["sort", "trier", "tri", "ordonner"],
    &["replace", "remplacer", "substitute"],
    &["clean", "cleaner", "nettoyer", "nettoyeur", "nettoyage"],
    &["hash", "empreinte", "checksum", "sha256", "sha", "md5"],
    &["timestamp", "horodatage", "epoch", "unix"],
    &["unit", "units", "unite", "unites"],
    &["fuel", "carburant", "essence", "gasoline"],
    &["age", "anniversaire", "birthday"],
    &[
        "case",
        "casse",
        "majuscule",
        "majuscules",
        "minuscule",
        "minuscules",
        "uppercase",
        "lowercase",
    ],
    &[
        "format",
        "formater",
        "formatter",
        "formateur",
        "beautify",
        "embellir",
        "indenter",
    ],
    &["qr", "qrcode", "flashcode"],
    &["uuid", "guid", "identifiant"],
];

fn group_of(word: &str) -> Option<usize> {
    SYNONYMS.iter().position(|g| g.contains(&word))
}

/// Query/document words: folded, stop words removed, single letters dropped.
#[must_use]
pub fn tokens(text: &str) -> Vec<String> {
    fold(text)
        .split_whitespace()
        .filter(|w| {
            !STOPWORDS.contains(w) && (w.chars().count() > 1 || w.chars().all(char::is_numeric))
        })
        .map(str::to_owned)
        .collect()
}

/// Optimal-string-alignment distance (insert, delete, substitute, adjacent swap).
#[must_use]
pub fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut rows = vec![vec![0_usize; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in rows[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = best;
        }
    }
    rows[a.len()][b.len()]
}

fn singular(word: &str) -> &str {
    word.strip_suffix('s')
        .or_else(|| word.strip_suffix('x'))
        .filter(|w| w.chars().count() >= 3)
        .unwrap_or(word)
}

/// Quality of matching query word `q` against document word `t`, in percent (0 = no match).
#[must_use]
pub fn word_quality(q: &str, t: &str) -> u32 {
    if q == t {
        return 100;
    }
    if singular(q) == singular(t) {
        return 95;
    }
    if let (Some(a), Some(b)) = (group_of(q), group_of(t))
        && a == b
    {
        return 90;
    }
    let (qs, ts) = (singular(q), singular(t));
    if qs.chars().count() >= 3 && ts.starts_with(qs) {
        return 70;
    }
    if ts.chars().count() >= 4 && qs.starts_with(ts) {
        return 60;
    }
    let len = qs.chars().count().min(ts.chars().count());
    let allowed = match len {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    };
    if allowed > 0 && distance(qs, ts) <= allowed {
        return 50;
    }
    0
}

const NAME_WEIGHT: u32 = 10;
/// Bonus for a query word that only the variant's own name explains ("16", "pin").
const VARIANT_SPECIFIC_BONUS: u32 = 6;
const KEYWORD_WEIGHT: u32 = 8;
const INTENT_WEIGHT: u32 = 5;
const UNIVERSE_WEIGHT: u32 = 3;
const ORDER_BONUS: u32 = 5;
/// Below this, a hit is noise.
const MIN_SCORE: u32 = 6;

/// Bonus when the query words appear, in the same order, in one phrase (a name or a keyword):
/// "png en jpg" prefers "PNG en JPG" over "JPG en PNG".
fn ordered_phrase_bonus(words: &[String], texts: &[String]) -> u32 {
    if words.len() < 2 {
        return 0;
    }
    let in_order = |text: &String| {
        let phrase = tokens(text);
        let mut from = 0;
        words.iter().all(|word| {
            phrase[from..]
                .iter()
                .position(|t| word_quality(word, t) >= 90)
                .is_some_and(|at| {
                    from += at + 1;
                    true
                })
        })
    };
    if texts.iter().any(in_order) {
        ORDER_BONUS
    } else {
        0
    }
}

fn best_in(query: &str, texts: &[String], weight: u32) -> u32 {
    texts
        .iter()
        .flat_map(|text| tokens(text))
        .map(|word| weight * word_quality(query, &word) / 100)
        .max()
        .unwrap_or(0)
}

fn list(map: &BTreeMap<String, Vec<String>>, lang: &str) -> Vec<String> {
    // The query language first, English as a fallback so a missing translation still finds it.
    let mut out: Vec<String> = map.get(lang).cloned().unwrap_or_default();
    if lang != "en" {
        out.extend(map.get("en").cloned().unwrap_or_default());
    }
    out
}

/// Score and whether a variant earned it through what is specific to the variant.
fn score(doc: &Doc, words: &[String], lang: &str) -> Option<(u32, bool)> {
    let name = doc
        .names
        .get(lang)
        .or_else(|| doc.names.get("en"))
        .cloned()
        .into_iter()
        .collect::<Vec<_>>();
    let keywords = list(&doc.keywords, lang);
    let intents = list(&doc.intents, lang);
    let universe = vec![doc.universe.clone()];
    // For a variant: the words of its name that its tool's name does not contain.
    let specific_tokens: Vec<String> = if doc.variant.is_some() {
        let tool_tokens: Vec<String> = doc
            .tool_names
            .get(lang)
            .or_else(|| doc.tool_names.get("en"))
            .map(|n| tokens(n))
            .unwrap_or_default();
        name.iter()
            .flat_map(|n| tokens(n))
            .filter(|t| tool_tokens.iter().all(|tool| word_quality(t, tool) < 70))
            .collect()
    } else {
        Vec::new()
    };
    let (mut total, mut matched, mut specific) = (0, 0, false);
    for word in words {
        let by_name = best_in(word, &name, NAME_WEIGHT);
        let by_tool = best_in(word, &keywords, KEYWORD_WEIGHT)
            .max(best_in(word, &intents, INTENT_WEIGHT))
            .max(best_in(word, &universe, UNIVERSE_WEIGHT));
        let mut best = by_name.max(by_tool);
        if specific_tokens.iter().any(|t| word_quality(word, t) >= 70) {
            best += VARIANT_SPECIFIC_BONUS;
            specific = true;
        }
        if best > 0 {
            total += best;
            matched += 1;
        }
    }
    let covered =
        u32::try_from(matched).unwrap_or(0) * 100 / u32::try_from(words.len()).unwrap_or(1);
    // Long requests are described by several words: demand a share of them, not all. The minimum
    // score still keeps weak single-word coincidences out.
    let needed = match words.len() {
        0..=2 => 100,
        3..=4 => 40,
        _ => 20,
    };
    if total < MIN_SCORE || covered < needed {
        return None;
    }
    // The name's word order counts double: variants share their tool's keywords (both directions).
    let order = 2 * ordered_phrase_bonus(words, &name) + ordered_phrase_bonus(words, &keywords);
    Some((
        total + order + if covered == 100 { 10 } else { 0 },
        specific,
    ))
}

/// Best hits for `query` in `lang`, most relevant first: at most `limit` entries and at most two
/// variants per tool. An empty or stop-word-only query returns nothing.
#[must_use]
pub fn search(docs: &[Doc], query: &str, lang: &str, limit: usize) -> Vec<Hit> {
    let words = tokens(query);
    if words.is_empty() || limit == 0 {
        return Vec::new();
    }
    let scored: Vec<(&Doc, u32, bool)> = docs
        .iter()
        .filter_map(|doc| score(doc, &words, lang).map(|(score, specific)| (doc, score, specific)))
        .collect();
    let tool_scores: BTreeMap<&str, u32> = scored
        .iter()
        .filter(|(doc, _, _)| doc.variant.is_none())
        .map(|(doc, score, _)| (doc.id.as_str(), *score))
        .collect();
    let mut hits: Vec<Hit> = scored
        .iter()
        .map(|(doc, score, specific)| {
            // A variant never outranks its tool unless the request is specific to the variant.
            let capped = match (&doc.variant, specific, tool_scores.get(doc.id.as_str())) {
                (Some(_), false, Some(tool)) => (*score).min(*tool),
                _ => *score,
            };
            Hit {
                id: doc.id.clone(),
                variant: doc.variant.clone(),
                score: capped,
            }
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.variant.is_some().cmp(&b.variant.is_some()))
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.variant.cmp(&b.variant))
    });
    let mut per_tool: BTreeMap<String, usize> = BTreeMap::new();
    hits.retain(|hit| {
        let count = per_tool.entry(hit.id.clone()).or_default();
        *count += 1;
        hit.variant.is_none() || *count <= 3
    });
    hits.truncate(limit);
    hits
}
