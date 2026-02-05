use std::fs;
use std::path::Path;
use regex::{Regex, self};
use serde::Serialize;
use std::fmt;
use chrono::{NaiveDateTime, NaiveDate, Duration};
use std::collections::HashMap;
use web_server::entities_v2::trace::NewTrace;
use uuid::Uuid;
use web_server::db;
use web_server::db::DbPool;

const USER_ID: Uuid = uuid::uuid!("a647a452-3a80-4a94-97d0-8a36c1c752c1");
const JOURNAL_ID: Uuid = uuid::uuid!("95bacf39-5ca7-4a9f-99c1-eb6d440dade1");

const DAYS: [&str; 7] = ["Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi", "Dimanche"];
const MONTHS: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
const YEARS: [&str; 8] = ["2020", "2021", "2022", "2023", "2024", "2025", "2026", "2027"];

fn read_to_string<P: AsRef<Path>>(path: P) -> anyhow::Result<String> {
    Ok(fs::read_to_string(path)?)
}

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub header: String,
    pub content: String,
    pub date: Option<String>,
}

impl Block {
    pub fn new() -> Self {
        Self { header: String::new(), content: String::new(), date: None }
    }
}
impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Header: {}\n\nContent: {}\n\nDate: {:?}\n\n", self.header, self.content, self.date)
    }
}

fn months_map() -> HashMap<&'static str, usize> {
    let mut months_map = HashMap::new();
    for (i, month) in MONTHS.iter().enumerate() {
        months_map.insert(*month, i + 1);
    }
    months_map
}
fn year_from_month(month: &str) -> Option<&str> {
    if month == "janvier" {
        return Some("2026");
    } else if month == "février" {
        return Some("2026");
    }
    return Some("2025");
}

fn french_date_regex() -> Regex {
    let days = DAYS
        .iter()
        .map(|d| regex::escape(d))
        .collect::<Vec<String>>()
        .join("|");
    let months = MONTHS
        .iter()
        .map(|m| regex::escape(m))
        .collect::<Vec<String>>()
        .join("|");
    let years = YEARS
        .iter()
        .map(|y| regex::escape(y))
        .collect::<Vec<String>>()
        .join("|");
    //Regex::new(format!(r"^(({})\s[0-9]{{1,2}}\s({})\s([0-9]{{4}}))$", days, months).as_str()).unwrap()
    //return Regex::new(r"((Lundi|Mardi|Mercredi|Jeudi|Vendredi|Samedi|Dimanche)\s[0-9]{1,2}\s(Janvier|Février|Mars|Avril|Mai|Juin|Juillet|Août|Septembre|Octobre|Novembre|Décembre|janvier|février|mars|avril|mai|juin|juillet|août|septembre|octobre|novembre|décembre)\s([0-9]{4}))").unwrap();
    return Regex::new(
        r"^(Lundi|Mardi|Mercredi|Jeudi|Vendredi|Samedi|Dimanche)\s+(?P<day>\d{1,2})(?:er)?\s+(?P<month>janvier|février|fevrier|mars|avril|mai|juin|juillet|août|aout|septembre|octobre|novembre|décembre|decembre)(\s+(?P<year>\d{4}))?(\s+(?P<hour>\d{1,2})(h|:)(?P<minute>\d{2}))?$"
    ).unwrap();
}

fn extract_date(date_option_string: Option<String>) -> Option<NaiveDateTime> {
    if let Some(date) = date_option_string {
        let date_time = NaiveDateTime::parse_from_str(&date, "%Y-%m-%d %H:%M");
        if date_time.is_err() {
            let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d");
            if date.is_err() {
                return None;
            }
            return Some(date.unwrap().and_hms_opt(12, 0, 0).unwrap());
        }
        return Some(date_time.unwrap());
    }
    None
}

fn is_start_of_block(line: &str) -> bool {
    line.starts_with("###") | french_date_regex().is_match(line)
}

fn get_block_date(line: &str) -> Option<String> {
    println!("Line: {}", line);
    let date_regex = Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap();
    let date_time_regex = Regex::new(r"(\d{4}-\d{2}-\d{2} \d{2}:\d{2})").unwrap();
    let french_date_regex = french_date_regex();
    if let Some(date_time) = date_time_regex.captures(line) {
        return Some(date_time[0].to_string());
    } else if let Some(date) = date_regex.captures(line) {
        return Some(date[0].to_string());
    } else if let Some(date) = french_date_regex.captures(line) {
        println!("Date: {:?}", date);
        if let (Some(hour), Some(minute)) = (date.name("hour"), date.name("minute")) {
            return Some(format!("{}-{}-{} {}:{}", 
            date.name("year").map(|y| y.as_str()).unwrap_or(year_from_month(&date["month"]).unwrap()), 
            months_map().get(&date["month"]).unwrap_or(&0), 
            date["day"].to_string(),
            hour.as_str(),
            minute.as_str()));
        } else {
            return Some(format!("{}-{}-{}", 
            date.name("year").map(|y| y.as_str()).unwrap_or(year_from_month(&date["month"]).unwrap()), 
            months_map().get(&date["month"]).unwrap_or(&0), 
            date["day"].to_string()));
        }
    } else {
        return None;
    }
}

fn extract_blocks(lines: Vec<&str>) -> Vec<Block> {
    let french_date_regex = french_date_regex();
    let mut last_empty = true;
    let mut blocks = Vec::new();
    let mut current_block = Block::new();
    for line in lines {
        println!("Line: {}", line);
        println!("Last empty: {}", last_empty);
        println!("Line starts with ###: {}", line.starts_with("###"));
        println!("French date regex {:?} matches line {:?}: {}", french_date_regex, line, french_date_regex.is_match(line));
        if line.is_empty() {
            last_empty = true;
            continue;
        } else if last_empty && is_start_of_block(line) {
            println!("Adding block");
            current_block.content = current_block.content.trim_matches('\n').to_string();
            blocks.push(current_block.clone());
            current_block = Block::new();
            current_block.header = line.to_string();
            current_block.date = get_block_date(line);
        }
        current_block.content += line;
        current_block.content += "\n";
        last_empty = false;
    }
    blocks.push(current_block.clone());
    blocks
}

fn blocks_to_traces(blocks: Vec<Block>) -> Vec<NewTrace> {
    let mut traces = Vec::new();
    let mut previous_date = None;
    for block in blocks {
        if block.header.is_empty() {
            continue;
        }
        println!("Block header : {:?}\n\n", block.header.clone());
        println!("Block date : {:?}\n\n", block.date.clone());
        let mut date = extract_date(block.date);
        if let Some(date) = date {
            previous_date = Some(date);
        } else if previous_date.is_some() {
            previous_date = Some(previous_date.unwrap() + Duration::days(1));
            date = previous_date;
        }
        println!("Date: {:?}", date);
        traces.push(NewTrace::new(
            "".to_string(),
            "".to_string(),
            block.content,
            date,
            USER_ID,
            JOURNAL_ID,
        ));
    }
    traces
}

fn create_traces(traces: Vec<NewTrace>, pool: &DbPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for trace in traces {
        println!("Creating trace with user id: {} and journal id: {}", trace.user_id, trace.journal_id);
        let result =trace.create(pool);
        match result {
            Ok(_) => println!("Trace created successfully"),
            Err(e) => println!("Error creating trace: {:?}", e),
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let pool = db::create_pool();
    db::init_global_pool(pool.clone());

    let file_path = Path::new("./imports/log_lecture.txt");

    let file_content = read_to_string(file_path)?;
    let lines = file_content.lines().collect::<Vec<&str>>();
    let blocks = extract_blocks(lines);
    let new_traces = blocks_to_traces(blocks);
    create_traces(new_traces, &pool)?;

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_blocks() {
        let lines = vec!["### 2026-02-04", "Mardi 3 décembre", "Mardi 2 décembre 2026", "This is a test", "### 2026-02-05", "This is a test", "### 2026-02-06", "This is a test"];
        let date = get_block_date(lines[1]);
        println!("Date: {:?}", date);
        assert_eq!(date, Some("2025-12-3".to_string()));
    }
}