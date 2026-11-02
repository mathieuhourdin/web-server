//! Keep PostgreSQL conversions compatible with client/chrono legacy IANA names.
const ALIASES: &[(&str, &str)] = &[
    ("Asia/Calcutta", "Asia/Kolkata"),
    ("Asia/Saigon", "Asia/Ho_Chi_Minh"),
];

pub(crate) fn postgres_timezone_name(tz: chrono_tz::Tz) -> &'static str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == tz.name())
        .map(|(_, canonical)| *canonical)
        .unwrap_or(tz.name())
}

/// `column` is a trusted SQL expression, never client input. The membership
/// subquery is uncorrelated, so PostgreSQL can hash its result once per query.
pub(crate) fn postgres_timezone_sql(column: &str) -> String {
    let aliases = ALIASES
        .iter()
        .map(|(alias, canonical)| format!("WHEN {column} = '{alias}' THEN '{canonical}'"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("CASE {aliases} WHEN {column} IN (SELECT name FROM pg_timezone_names) THEN {column} ELSE 'UTC' END")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_legacy_names_without_changing_regular_zones() {
        for (input, expected) in [
            ("Asia/Calcutta", "Asia/Kolkata"),
            ("Asia/Saigon", "Asia/Ho_Chi_Minh"),
            ("Europe/Paris", "Europe/Paris"),
        ] {
            assert_eq!(postgres_timezone_name(input.parse().unwrap()), expected);
            if input != expected {
                assert!(postgres_timezone_sql("u.timezone").contains(expected));
            }
        }
    }
}
