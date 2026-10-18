use std::collections::BTreeMap;

use kortex_seek::engine::{Doc, distance, fold, search, tokens, word_quality};

fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn lists(fr: &[&str], en: &[&str]) -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([
        (
            "fr".to_owned(),
            fr.iter().map(|s| (*s).to_owned()).collect(),
        ),
        (
            "en".to_owned(),
            en.iter().map(|s| (*s).to_owned()).collect(),
        ),
    ])
}

fn catalogue() -> Vec<Doc> {
    vec![
        Doc {
            id: "image.convert".into(),
            universe: "image".into(),
            names: map(&[("fr", "Convertisseur d'images"), ("en", "Image Converter")]),
            keywords: lists(
                &["convertir image", "png en jpg", "webp"],
                &["convert image", "png to jpg", "webp"],
            ),
            intents: lists(
                &["convertir une image png en jpg"],
                &["convert a png image to jpg"],
            ),
            ..Doc::default()
        },
        Doc {
            id: "image.convert".into(),
            variant: Some("png-to-jpg".into()),
            universe: "image".into(),
            names: map(&[("fr", "Convertir PNG en JPG"), ("en", "Convert PNG to JPG")]),
            tool_names: map(&[("fr", "Convertisseur d'images"), ("en", "Image Converter")]),
            keywords: lists(
                &["convertir image", "png en jpg"],
                &["convert image", "png to jpg"],
            ),
            ..Doc::default()
        },
        Doc {
            id: "image.convert".into(),
            variant: Some("jpg-to-png".into()),
            universe: "image".into(),
            names: map(&[("fr", "Convertir JPG en PNG"), ("en", "Convert JPG to PNG")]),
            tool_names: map(&[("fr", "Convertisseur d'images"), ("en", "Image Converter")]),
            keywords: lists(
                &["convertir image", "jpg en png"],
                &["convert image", "jpg to png"],
            ),
            ..Doc::default()
        },
        Doc {
            id: "secure.password-generator".into(),
            universe: "secure".into(),
            names: map(&[
                ("fr", "Générateur de mots de passe"),
                ("en", "Password Generator"),
            ]),
            keywords: lists(
                &["mot de passe", "sécurisé", "aléatoire"],
                &["password", "random", "strong"],
            ),
            intents: lists(
                &["créer un mot de passe sécurisé"],
                &["create a strong password"],
            ),
            ..Doc::default()
        },
        Doc {
            id: "secure.password-generator".into(),
            variant: Some("16".into()),
            universe: "secure".into(),
            names: map(&[
                ("fr", "Mot de passe de 16 caractères"),
                ("en", "16 character password"),
            ]),
            tool_names: map(&[
                ("fr", "Générateur de mots de passe"),
                ("en", "Password Generator"),
            ]),
            keywords: lists(
                &["mot de passe", "sécurisé", "aléatoire"],
                &["password", "random", "strong"],
            ),
            intents: lists(
                &["créer un mot de passe sécurisé"],
                &["create a strong password"],
            ),
        },
        Doc {
            id: "pdf.merge".into(),
            universe: "pdf".into(),
            names: map(&[("fr", "Fusionner des PDF"), ("en", "Merge PDF")]),
            keywords: lists(
                &["fusionner pdf", "assembler pdf"],
                &["merge pdf", "combine pdf"],
            ),
            ..Doc::default()
        },
        Doc {
            id: "text.word-counter".into(),
            universe: "text".into(),
            names: map(&[("fr", "Compteur de mots"), ("en", "Word Counter")]),
            keywords: lists(
                &["compter les mots", "caractères"],
                &["word count", "characters"],
            ),
            ..Doc::default()
        },
    ]
}

fn top(query: &str, lang: &str) -> Option<(String, Option<String>)> {
    search(&catalogue(), query, lang, 5)
        .into_iter()
        .next()
        .map(|h| (h.id, h.variant))
}

#[test]
fn folds_accents_ligatures_and_punctuation() {
    assert_eq!(fold("Générateur d'œuvres!"), "generateur d oeuvres ");
    assert_eq!(
        tokens("Je veux créer un mot de passe"),
        ["creer", "mot", "passe"]
    );
}

#[test]
fn finds_a_tool_from_a_natural_request_in_both_languages() {
    assert_eq!(
        top("créer un mot de passe sécurisé", "fr").map(|h| h.0),
        Some("secure.password-generator".into())
    );
    assert_eq!(
        top("generate a strong password", "en").map(|h| h.0),
        Some("secure.password-generator".into())
    );
    assert_eq!(
        top("assembler des documents pdf", "fr").map(|h| h.0),
        Some("pdf.merge".into())
    );
    assert_eq!(
        top("mdp", "fr").map(|h| h.0),
        Some("secure.password-generator".into())
    );
}

#[test]
fn tolerates_typos_plurals_and_accent_free_input() {
    assert_eq!(
        top("convertiseur image", "fr").map(|h| h.0),
        Some("image.convert".into())
    );
    assert_eq!(
        top("generateur mots passe", "fr").map(|h| h.0),
        Some("secure.password-generator".into())
    );
    assert_eq!(
        top("fusioner pdf", "fr").map(|h| h.0),
        Some("pdf.merge".into())
    );
    assert!(distance("fusionner", "fusioner") <= 2);
    assert!(word_quality("images", "image") >= 90);
    assert_eq!(word_quality("xyz", "image"), 0);
}

#[test]
fn a_specific_request_opens_the_matching_variant() {
    assert_eq!(
        top("png en jpg", "fr"),
        Some(("image.convert".into(), Some("png-to-jpg".into())))
    );
    assert_eq!(
        top("convert png to jpg", "en"),
        Some(("image.convert".into(), Some("png-to-jpg".into())))
    );
    // A generic request still prefers the tool itself over a preset.
    assert_eq!(
        top("convertisseur", "fr"),
        Some(("image.convert".into(), None))
    );
}

#[test]
fn nothing_is_returned_for_empty_noise_or_unrelated_requests() {
    assert!(search(&catalogue(), "", "fr", 5).is_empty());
    assert!(search(&catalogue(), "   le la les", "fr", 5).is_empty());
    assert!(search(&catalogue(), "zzzqqq", "fr", 5).is_empty());
    assert!(search(&catalogue(), "mot de passe", "fr", 0).is_empty());
}

#[test]
fn long_requests_need_only_a_share_of_their_words() {
    let hits = search(
        &catalogue(),
        "réduire le poids d'une photo pour l'envoyer par mail",
        "fr",
        5,
    );
    assert_eq!(hits.first().map(|h| h.id.as_str()), Some("image.convert"));
}

#[test]
fn results_are_limited_and_stable() {
    let docs = catalogue();
    let first = search(&docs, "pdf image mot", "fr", 2);
    assert!(first.len() <= 2);
    assert_eq!(first, search(&docs, "pdf image mot", "fr", 2));
}

#[test]
fn word_order_picks_the_right_direction_of_a_conversion() {
    assert_eq!(
        top("png en jpg", "fr"),
        Some(("image.convert".into(), Some("png-to-jpg".into())))
    );
    assert_eq!(
        top("jpg en png", "fr"),
        Some(("image.convert".into(), Some("jpg-to-png".into())))
    );
    assert_eq!(
        top("jpg to png", "en"),
        Some(("image.convert".into(), Some("jpg-to-png".into())))
    );
}

#[test]
fn a_variant_wins_only_with_words_that_are_specific_to_it() {
    assert_eq!(
        top("mdp", "fr"),
        Some(("secure.password-generator".into(), None))
    );
    assert_eq!(
        top("mot de passe", "fr"),
        Some(("secure.password-generator".into(), None))
    );
    assert_eq!(
        top("mot de passe 16 caractères", "fr"),
        Some(("secure.password-generator".into(), Some("16".into())))
    );
    assert_eq!(
        top("16 character password", "en"),
        Some(("secure.password-generator".into(), Some("16".into())))
    );
}
