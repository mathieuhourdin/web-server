//! Explicit local maintenance command; never calls an AI provider.
use diesel::{
    prelude::*,
    sql_types::{BigInt, Text, Uuid as SqlUuid},
};
use uuid::Uuid;
use web_server::entities_v2::records::trace::language::{detect_language, store};

#[derive(QueryableByName)]
struct Candidate {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    content: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let mut batch_size = 200i64;
    let mut cursor = Uuid::nil();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--batch-size" => batch_size = args.next().ok_or("Missing batch size")?.parse()?,
            "--after" => cursor = args.next().ok_or("Missing UUID")?.parse()?,
            "--help" => {
                println!("backfill_trace_languages [--batch-size 1..1000] [--after UUID]");
                return Ok(());
            }
            _ => return Err("Unknown argument; use --help".into()),
        }
    }
    if !(1..=1000).contains(&batch_size) {
        return Err("Batch size must be 1..1000".into());
    }
    let mut conn = PgConnection::establish(&std::env::var("DATABASE_URL")?)?;
    let mut scanned = 0usize;
    let mut detected = 0usize;
    loop {
        let (last, count, found) = conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let rows = diesel::sql_query(
                "SELECT id, left(title,16384) AS title, left(content,16384) AS content
                 FROM traces WHERE id > $1 AND language_code IS NULL AND NOT is_encrypted
                   AND status IN ('FINALIZED','ARCHIVED') AND finalized_at IS NOT NULL
                 ORDER BY id LIMIT $2 FOR UPDATE",
            )
            .bind::<SqlUuid, _>(cursor)
            .bind::<BigInt, _>(batch_size)
            .load::<Candidate>(conn)?;
            let mut found = 0;
            for row in &rows {
                if let Some(code) = detect_language(&row.title, &row.content) {
                    found += store(conn, row.id, Some(&code))?;
                }
            }
            Ok((rows.last().map(|r| r.id), rows.len(), found))
        })?;
        scanned += count;
        detected += found;
        let Some(last) = last else {
            break;
        };
        cursor = last;
        // Aggregate progress only: no titles, text, or connection credentials.
        println!("scanned={scanned} detected={detected} resume_after={cursor}");
    }
    println!(
        "Finished: scanned={scanned} detected={detected} unknown={}",
        scanned - detected
    );
    Ok(())
}
