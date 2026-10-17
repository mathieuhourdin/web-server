//! Local, bounded language detection. No provider, AI consent or quota dependency.
use super::model::{Trace, TraceStatus};
use diesel::{
    prelude::*,
    sql_types::{Bool, Nullable, Text, Uuid as SqlUuid},
};
use regex::Regex;
use std::sync::OnceLock;
use uuid::Uuid;

const SAMPLE_CHARS: usize = 16_384;
const MIN_LETTERS: usize = 20;

pub fn detect_language(title: &str, content: &str) -> Option<String> {
    let source = if content.trim().is_empty() {
        title
    } else {
        content
    };
    let sample = source.chars().take(SAMPLE_CHARS).collect::<String>();
    static NOISE: OnceLock<Regex> = OnceLock::new();
    let noise =
        NOISE.get_or_init(|| Regex::new(r"(?s)```.*?```|https?://\S+|www\.\S+|<[^>]*>").unwrap());
    let text = noise.replace_all(&sample, " ");
    if text
        .chars()
        .filter(|c| c.is_alphabetic())
        .take(MIN_LETTERS)
        .count()
        < MIN_LETTERS
    {
        return None;
    }
    let result = whatlang::detect(&text)?;
    if !result.is_reliable() {
        return None;
    }
    Some(language_code(result.lang()).to_string())
}

// ISO 639-1 where available. Normalize detector-specific Mandarin/Iranian Persian
// categories to the broad Chinese/Persian language codes used by clients.
fn language_code(lang: whatlang::Lang) -> &'static str {
    use whatlang::Lang::*;
    match lang {
        Epo => "eo",
        Eng => "en",
        Rus => "ru",
        Cmn => "zh",
        Spa => "es",
        Por => "pt",
        Ita => "it",
        Ben => "bn",
        Fra => "fr",
        Deu => "de",
        Ukr => "uk",
        Kat => "ka",
        Ara => "ar",
        Hin => "hi",
        Jpn => "ja",
        Heb => "he",
        Yid => "yi",
        Pol => "pl",
        Amh => "am",
        Jav => "jv",
        Kor => "ko",
        Nob => "nb",
        Dan => "da",
        Swe => "sv",
        Fin => "fi",
        Tur => "tr",
        Nld => "nl",
        Hun => "hu",
        Ces => "cs",
        Ell => "el",
        Bul => "bg",
        Bel => "be",
        Mar => "mr",
        Kan => "kn",
        Ron => "ro",
        Slv => "sl",
        Hrv => "hr",
        Srp => "sr",
        Mkd => "mk",
        Lit => "lt",
        Lav => "lv",
        Est => "et",
        Tam => "ta",
        Vie => "vi",
        Urd => "ur",
        Tha => "th",
        Guj => "gu",
        Uzb => "uz",
        Pan => "pa",
        Aze => "az",
        Ind => "id",
        Tel => "te",
        Pes => "fa",
        Mal => "ml",
        Ori => "or",
        Mya => "my",
        Nep => "ne",
        Sin => "si",
        Khm => "km",
        Tuk => "tk",
        Aka => "ak",
        Zul => "zu",
        Sna => "sn",
        Afr => "af",
        Lat => "la",
        Slk => "sk",
        Cat => "ca",
        Tgl => "tl",
        Hye => "hy",
        Cym => "cy",
    }
}

#[derive(QueryableByName)]
struct Previous {
    #[diesel(sql_type=Text)]
    title: String,
    #[diesel(sql_type=Text)]
    content: String,
    #[diesel(sql_type=Text)]
    status: String,
    #[diesel(sql_type=Bool)]
    is_encrypted: bool,
    #[diesel(sql_type=Nullable<Text>)]
    language_code: Option<String>,
}

/// Called inside the existing write transaction. Metadata-only edits retain the
/// previous result (including an intentional unknown), avoiding repeat detection.
pub(super) fn for_update(trace: &Trace, conn: &mut PgConnection) -> QueryResult<Option<String>> {
    if trace.is_encrypted || trace.status == TraceStatus::Draft {
        return Ok(None);
    }
    let previous = diesel::sql_query(
        "SELECT title,content,status,is_encrypted,language_code FROM traces WHERE id=$1 FOR UPDATE",
    )
    .bind::<SqlUuid, _>(trace.id)
    .get_result::<Previous>(conn)
    .optional()?;
    if let Some(previous) = previous {
        let old_text = if previous.content.trim().is_empty() {
            &previous.title
        } else {
            &previous.content
        };
        let new_text = if trace.content.trim().is_empty() {
            &trace.title
        } else {
            &trace.content
        };
        if previous.status != "DRAFT" && !previous.is_encrypted && old_text == new_text {
            return Ok(previous.language_code);
        }
    }
    Ok(detect_language(&trace.title, &trace.content))
}

/// Metadata-only write for insertion/backfill: never creates an edit version or
/// requeues analysis. Existing text is not printed or sent outside the process.
pub fn store(conn: &mut PgConnection, id: Uuid, code: Option<&str>) -> QueryResult<usize> {
    diesel::sql_query(
        "UPDATE traces SET language_code=$2 WHERE id=$1 AND language_code IS DISTINCT FROM $2",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<Nullable<Text>, _>(code)
    .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_detection_is_local_bounded_and_conservative() {
        assert_eq!(detect_language("","Je me suis promené dans la forêt ce matin. Les arbres étaient magnifiques et j'ai pris le temps de regarder les oiseaux."),Some("fr".into()));
        assert_eq!(detect_language("","I spent the afternoon walking through the countryside and thinking about the book I have been reading this week."),Some("en".into()));
        for text in [
            "",
            "OK",
            "❤️🎉 1234",
            "https://example.com/a-long-english-looking-url",
            "```some computer program with many ordinary words```",
        ] {
            assert_eq!(detect_language("", text), None);
        }
    }
}
