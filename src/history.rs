use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, SecondsFormat, Utc};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Legacy rows have three columns; new rows carry an unambiguous UTC instant,
/// the recorded offset and an executable identity. Never guess missing old data.
pub struct Usage {
    pub local_start: NaiveDateTime,
    pub identity: String,
    pub seconds: f64,
}

pub fn identity(path: &Path) -> String {
    let path = path.to_string_lossy().replace('/', "\\");
    format!(
        "path:{}",
        path.strip_prefix("\\\\?\\").unwrap_or(&path).to_lowercase()
    )
}

pub fn executable_path(id: &str) -> Option<PathBuf> {
    id.strip_prefix("path:").map(PathBuf::from)
}

pub fn executable_name(id: &str) -> String {
    executable_path(id)
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| id.to_string())
}

pub fn encode(start: &DateTime<Local>, id: &str, path: Option<&Path>, seconds: f64) -> Vec<u8> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer
        .write_record([
            start
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Micros, true),
            executable_name(id),
            format!("{seconds:.6}"),
            id.to_string(),
            path.map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            start.offset().local_minus_utc().to_string(),
        ])
        .expect("CSV en memoria");
    writer.into_inner().expect("CSV en memoria")
}

pub fn parse(row: &csv::StringRecord) -> Option<Usage> {
    let seconds = row.get(2)?.parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let (local_start, identity) = match row.len() {
        3 => (
            NaiveDateTime::parse_from_str(&row[0], "%Y-%m-%d %H:%M:%S").ok()?,
            row[1].to_string(),
        ),
        6 => {
            let utc = DateTime::parse_from_rfc3339(&row[0]).ok()?;
            let offset = FixedOffset::east_opt(row[5].parse().ok()?)?;
            let id = row[3].to_string();
            if id.is_empty() || (!row[4].is_empty() && identity(Path::new(&row[4])) != id) {
                return None;
            }
            (utc.with_timezone(&offset).naive_local(), id)
        }
        _ => return None,
    };
    Some(Usage {
        local_start,
        identity,
        seconds,
    })
}

pub fn applications<'a>(ids: impl Iterator<Item = &'a str>) -> serde_json::Value {
    let unique: BTreeMap<_, _> = ids
        .map(|id| (id.to_string(), executable_name(id)))
        .collect();
    let mut names = BTreeMap::<String, usize>::new();
    for name in unique.values() {
        *names.entry(name.to_lowercase()).or_default() += 1;
    }
    let values: serde_json::Map<_, _> = unique
        .into_iter()
        .map(|(id, name)| {
            let path = executable_path(&id);
            let suffix = if names[&name.to_lowercase()] > 1 {
                Some(
                    path.as_ref()
                        .and_then(|path| path.parent())
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "historial anterior".into()),
                )
            } else {
                None
            };
            (
                id,
                serde_json::json!({"name":name,"path":path,"suffix":suffix}),
            )
        })
        .collect();
    values.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn partial_new_format_rows_recover_idempotently_after_legacy_history() {
        let legacy = b"2026-10-14 10:00:00,old.exe,0.5\n";
        let path = Path::new(r"C:\Apps, Unicode\app.exe");
        let row = encode(&Local::now(), &identity(path), Some(path), 0.125);
        for length in [0, 1, row.len() / 2, row.len()] {
            let fixture = crate::test_support::Fixture::new("");
            let mut partial = legacy.to_vec();
            partial.extend_from_slice(&row[..length]);
            std::fs::write(&fixture.csv, partial).unwrap();
            let pending = serde_json::json!({"offset":legacy.len(),"row":base64::engine::general_purpose::STANDARD.encode(&row)});
            std::fs::write(
                fixture.csv.with_extension("pending.json"),
                serde_json::to_vec(&pending).unwrap(),
            )
            .unwrap();
            crate::durability::Journal::open(&fixture.csv).unwrap();
            crate::durability::Journal::open(&fixture.csv).unwrap();
            let mut expected = legacy.to_vec();
            expected.extend_from_slice(&row);
            assert_eq!(std::fs::read(&fixture.csv).unwrap(), expected);
            let summary = crate::daily_summary::DailySummary::load(&fixture.csv).unwrap();
            assert!(summary.first_date().is_some());
        }
    }
    #[test]
    fn same_name_in_different_directories_has_distinct_stable_identity() {
        let a = identity(Path::new(r"C:\Apps\Uno\app.exe"));
        let b = identity(Path::new(r"C:\Apps\Dos\app.exe"));
        assert_ne!(a, b);
        assert_eq!(a, identity(Path::new(r"c:/APPS/uno/APP.EXE")));
        let apps = applications([a.as_str(), b.as_str(), "app.exe"].into_iter());
        assert_eq!(apps["app.exe"]["suffix"], "historial anterior");
        assert_ne!(apps[&a]["suffix"], apps[&b]["suffix"]);
    }
    #[test]
    fn utc_and_original_offset_roundtrip_fractional_seconds_and_csv_quoting() {
        let path = Path::new("C:\\Apps, prueba\\app.exe");
        let start = DateTime::parse_from_rfc3339("2026-10-15T23:59:59.123456-03:00")
            .unwrap()
            .with_timezone(&Local);
        let bytes = encode(&start, &identity(path), Some(path), 0.75);
        let row = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(bytes.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert!(row[0].ends_with('Z'));
        assert_eq!(
            DateTime::parse_from_rfc3339(&row[0])
                .unwrap()
                .timestamp_micros(),
            start.timestamp_micros()
        );
        let usage = parse(&row).unwrap();
        assert_eq!(usage.local_start, start.naive_local());
        assert_eq!(usage.seconds, 0.75);
        assert_eq!(usage.identity, identity(path));
    }
    #[test]
    fn repeated_wall_hour_retains_distinct_utc_instants() {
        let rows = [
            csv::StringRecord::from(vec![
                "2026-11-01T05:30:00Z",
                "app.exe",
                "60",
                "app.exe",
                "",
                "-14400",
            ]),
            csv::StringRecord::from(vec![
                "2026-11-01T06:30:00Z",
                "app.exe",
                "60",
                "app.exe",
                "",
                "-18000",
            ]),
        ];
        assert_eq!(
            parse(&rows[0]).unwrap().local_start,
            parse(&rows[1]).unwrap().local_start
        );
        assert_ne!(rows[0][0], rows[1][0]);
        let legacy = csv::StringRecord::from(vec!["2026-10-15 10:00:00", "app.exe", "0.25"]);
        assert_eq!(parse(&legacy).unwrap().identity, "app.exe");
    }
}
