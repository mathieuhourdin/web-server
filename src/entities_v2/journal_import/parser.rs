use chrono::{Duration, NaiveDate, NaiveDateTime};
use regex::Regex;
use uuid::Uuid;

use crate::entities_v2::trace::NewTrace;

use super::model::ImportBlock;

fn year_from_month(month: &str) -> i32 {
    match month {
        "janvier" | "fevrier" | "février" => 2026,
        _ => 2025,
    }
}

fn month_number(month: &str) -> Option<u32> {
    match month {
        "janvier" => Some(1),
        "fevrier" | "février" => Some(2),
        "mars" => Some(3),
        "avril" => Some(4),
        "mai" => Some(5),
        "juin" => Some(6),
        "juillet" => Some(7),
        "aout" | "août" => Some(8),
        "septembre" => Some(9),
        "octobre" => Some(10),
        "novembre" => Some(11),
        "decembre" | "décembre" => Some(12),
        _ => None,
    }
}

fn french_date_regex() -> Regex {
    Regex::new(
        r"^(?i)(lundi|mardi|mercredi|jeudi|vendredi|samedi|dimanche)\s+(?P<day>\d{1,2})(?:er)?\s+(?P<month>janvier|février|fevrier|mars|avril|mai|juin|juillet|août|aout|septembre|octobre|novembre|décembre|decembre)(?:\s+(?P<year>\d{4}))?(?:\s+(?P<hour>\d{1,2})(?:h|:)(?P<minute>\d{2}))?$",
    )
    .expect("valid regex")
}

fn strip_markdown_heading_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    let without_hashes = trimmed.trim_start_matches('#').trim_start();
    if without_hashes.is_empty() {
        trimmed
    } else {
        without_hashes
    }
}

fn get_block_date(line: &str) -> Option<String> {
    let normalized_line = strip_markdown_heading_prefix(line);
    let date_time_regex = Regex::new(r"(\d{4}-\d{2}-\d{2}\s\d{2}:\d{2})").expect("valid regex");
    let date_regex = Regex::new(r"(\d{4}-\d{2}-\d{2})").expect("valid regex");

    if let Some(date_time) = date_time_regex.captures(normalized_line) {
        return Some(date_time[1].to_string());
    }
    if let Some(date) = date_regex.captures(normalized_line) {
        return Some(date[1].to_string());
    }

    let captures = french_date_regex().captures(normalized_line)?;
    let day = captures.name("day")?.as_str().parse::<u32>().ok()?;
    let month_name = captures.name("month")?.as_str().to_lowercase();
    let month = month_number(&month_name)?;
    let year = captures
        .name("year")
        .and_then(|y| y.as_str().parse::<i32>().ok())
        .unwrap_or_else(|| year_from_month(&month_name));

    if let (Some(hour), Some(minute)) = (captures.name("hour"), captures.name("minute")) {
        let hour = hour.as_str().parse::<u32>().ok()?;
        let minute = minute.as_str().parse::<u32>().ok()?;
        return Some(format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            year, month, day, hour, minute
        ));
    }

    Some(format!("{:04}-{:02}-{:02}", year, month, day))
}

fn extract_date(value: Option<&str>) -> Option<NaiveDateTime> {
    let value = value?;
    if let Ok(date_time) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M") {
        return Some(date_time);
    }
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return date.and_hms_opt(12, 0, 0);
    }
    None
}

fn is_start_of_block(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') || get_block_date(trimmed).is_some()
}

pub fn extract_blocks(text: &str) -> Vec<ImportBlock> {
    let mut blocks = Vec::new();
    let mut current_block = ImportBlock {
        header: String::new(),
        content: String::new(),
        date: None,
    };
    let mut last_empty = true;

    for line in text.lines() {
        if line.is_empty() {
            last_empty = true;
            continue;
        }
        if last_empty && is_start_of_block(line) {
            current_block.content = current_block.content.trim_matches('\n').to_string();
            blocks.push(current_block);
            current_block = ImportBlock {
                header: line.to_string(),
                content: String::new(),
                date: get_block_date(line),
            };
        }
        current_block.content.push_str(line);
        current_block.content.push('\n');
        last_empty = false;
    }

    current_block.content = current_block.content.trim_matches('\n').to_string();
    blocks.push(current_block);
    blocks
}

pub fn blocks_to_new_traces(
    blocks: Vec<ImportBlock>,
    user_id: Uuid,
    journal_id: Uuid,
) -> Vec<(usize, ImportBlock, NewTrace)> {
    let mut traces = Vec::new();
    let mut previous_date: Option<NaiveDateTime> = None;

    for (block_index, block) in blocks.into_iter().enumerate() {
        if block.header.trim().is_empty() {
            continue;
        }

        let mut interaction_date = extract_date(block.date.as_deref());
        if let Some(date) = interaction_date {
            previous_date = Some(date);
        } else if let Some(previous) = previous_date {
            let next = previous + Duration::days(1);
            previous_date = Some(next);
            interaction_date = Some(next);
        }

        let trace = NewTrace::new(
            "".to_string(),
            "".to_string(),
            block.content.clone(),
            interaction_date,
            user_id,
            journal_id,
        );
        traces.push((block_index, block, trace));
    }

    traces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_french_date_with_explicit_year() {
        let date = get_block_date("Mardi 2 décembre 2026");
        assert_eq!(date, Some("2026-12-02".to_string()));
    }

    #[test]
    fn parses_french_date_without_year_with_fallback() {
        let date = get_block_date("Mardi 3 décembre");
        assert_eq!(date, Some("2025-12-03".to_string()));
    }

    #[test]
    fn parses_iso_date_and_datetime() {
        assert_eq!(
            get_block_date("### 2026-02-04"),
            Some("2026-02-04".to_string())
        );
        assert_eq!(
            get_block_date("header 2026-02-04 17:35"),
            Some("2026-02-04 17:35".to_string())
        );
    }

    #[test]
    fn extracts_blocks_with_blank_line_separators() {
        let text = "### 2026-02-04\nA\n\n### 2026-02-05\nB\n";
        let blocks = extract_blocks(text);
        let real_blocks = blocks
            .into_iter()
            .filter(|block| !block.header.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(real_blocks.len(), 2);
        assert_eq!(real_blocks[0].date, Some("2026-02-04".to_string()));
        assert_eq!(real_blocks[1].date, Some("2026-02-05".to_string()));
    }

    #[test]
    fn parses_markdown_heading_dates() {
        assert_eq!(
            get_block_date("## Mardi 2 décembre 2026"),
            Some("2026-12-02".to_string())
        );
        assert_eq!(
            get_block_date("# 2026-02-04 17:35"),
            Some("2026-02-04 17:35".to_string())
        );
    }

    #[test]
    fn missing_date_uses_previous_plus_one_day() {
        let user_id = Uuid::new_v4();
        let journal_id = Uuid::new_v4();
        let blocks = vec![
            ImportBlock {
                header: "### 2026-02-04".to_string(),
                content: "A".to_string(),
                date: Some("2026-02-04".to_string()),
            },
            ImportBlock {
                header: "### no date".to_string(),
                content: "B".to_string(),
                date: None,
            },
        ];
        let traces = blocks_to_new_traces(blocks, user_id, journal_id);
        assert_eq!(traces.len(), 2);
        assert_eq!(
            traces[0].2.interaction_date,
            NaiveDate::from_ymd_opt(2026, 2, 4).and_then(|d| d.and_hms_opt(12, 0, 0))
        );
        assert_eq!(
            traces[1].2.interaction_date,
            NaiveDate::from_ymd_opt(2026, 2, 5).and_then(|d| d.and_hms_opt(12, 0, 0))
        );
    }
}
