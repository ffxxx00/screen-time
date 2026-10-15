use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::add_daily_segment;

type DayUsage = BTreeMap<String, [f64; 24]>;

// The CSV is the source of truth. This file only stores a rebuildable index.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct DailySummary {
    version: u8,
    source_len: u64,
    source_mtime_ns: u128,
    source_hash: u64,
    #[serde(skip)]
    trusted_growth: u64,
    first_date: Option<NaiveDate>,
    days: BTreeMap<NaiveDate, DayUsage>,
}

impl DailySummary {
    pub fn load(csv_path: &Path) -> Result<Self, String> {
        let cache = cache_path(csv_path);
        let saved = fs::read(cache)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok());
        let mut summary = saved.filter(|saved| saved.version == 3).unwrap_or_default();
        if summary.version == 3 {
            let mut file = File::open(csv_path).map_err(|e| e.to_string())?;
            let length = file.metadata().map_err(|e| e.to_string())?.len();
            if length < summary.source_len
                || fingerprint(&mut file, summary.source_len).map_err(|e| e.to_string())?
                    != summary.source_hash
            {
                summary = Self::default();
            }
        }
        summary.sync(csv_path)?;
        Ok(summary)
    }

    pub fn day(&self, date: NaiveDate) -> DayUsage {
        self.days.get(&date).cloned().unwrap_or_default()
    }

    pub fn first_date(&self) -> Option<NaiveDate> {
        self.first_date
    }

    // Only call after our journal has durably appended this exact number of bytes.
    pub fn note_committed_append(&mut self, bytes: usize) {
        self.trusted_growth = self.trusted_growth.saturating_add(bytes as u64);
    }

    pub fn sync(&mut self, csv_path: &Path) -> Result<(), String> {
        let mut file =
            File::open(csv_path).map_err(|e| format!("No se pudo abrir el historial: {e}"))?;
        let metadata = file
            .metadata()
            .map_err(|e| format!("No se pudo consultar el historial: {e}"))?;
        let length = metadata.len();
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);

        if self.version == 3 && length == self.source_len && modified == self.source_mtime_ns {
            return Ok(());
        }

        let trusted_append = self.version == 3
            && self.trusted_growth > 0
            && length == self.source_len.saturating_add(self.trusted_growth);
        let can_append = self.version == 3
            && self.source_len > 0
            && length > self.source_len
            && (trusted_append
                || fingerprint(&mut file, self.source_len).map_err(|e| e.to_string())?
                    == self.source_hash);
        let mut next = if can_append {
            self.clone()
        } else {
            Self::default()
        };
        let offset = if can_append { self.source_len } else { 0 };
        if length > 0 {
            file.seek(SeekFrom::Start(length - 1))
                .map_err(|e| e.to_string())?;
            let mut last = [0];
            file.read_exact(&mut last).map_err(|e| e.to_string())?;
            if last[0] != b'\n' {
                return Err("El historial contiene una fila CSV incompleta.".into());
            }
        }
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(&mut file);
        for record in reader.records() {
            let record = record.map_err(|e| format!("Registro inválido: {e}"))?;
            if let Some(usage) = crate::history::parse(&record) {
                if usage.seconds > 0.0 && usage.seconds <= 30.0 * 86400.0 {
                    let start = usage.local_start;
                    next.first_date = Some(
                        next.first_date
                            .map_or(start.date(), |first| first.min(start.date())),
                    );
                    next.add_segment(start, &usage.identity, usage.seconds);
                }
            }
        }
        drop(reader);
        let source_hash = if trusted_append && can_append {
            fingerprint_from(&mut file, self.source_len, length, self.source_hash)
                .map_err(|e| e.to_string())?
        } else {
            fingerprint(&mut file, length).map_err(|e| e.to_string())?
        };
        next.version = 3;
        next.source_len = length;
        next.source_mtime_ns = modified;
        next.source_hash = source_hash;
        next.trusted_growth = 0;
        *self = next;
        // Failure to save the derived index must never affect the CSV or the dashboard.
        if let Ok(bytes) = serde_json::to_vec(self) {
            let _ = crate::durability::atomic_write(&cache_path(csv_path), &bytes);
        }
        Ok(())
    }

    fn add_segment(&mut self, start: NaiveDateTime, name: &str, mut seconds: f64) {
        let mut cursor = start;
        while seconds > 0.0 {
            let Some(next_date) = cursor.date().succ_opt() else {
                break;
            };
            let Some(next_midnight) = next_date.and_hms_opt(0, 0, 0) else {
                break;
            };
            let until_midnight =
                (next_midnight - cursor).num_microseconds().unwrap() as f64 / 1_000_000.0;
            let chunk = seconds.min(until_midnight);
            if chunk <= 0.0 {
                break;
            }
            add_daily_segment(
                self.days.entry(cursor.date()).or_default(),
                cursor.date(),
                cursor,
                name,
                chunk,
            );
            seconds -= chunk;
            cursor = next_midnight;
        }
    }
}

fn cache_path(csv_path: &Path) -> PathBuf {
    csv_path.with_extension("daily.json")
}

fn fingerprint(file: &mut File, length: u64) -> std::io::Result<u64> {
    fingerprint_from(file, 0, length, 0xcbf29ce484222325_u64)
}

fn fingerprint_from(file: &mut File, start: u64, end: u64, mut hash: u64) -> std::io::Result<u64> {
    file.seek(SeekFrom::Start(start))?;
    let mut remaining = end - start;
    let mut buffer = [0u8; 65536];
    while remaining > 0 {
        let width = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..width])?;
        for byte in &buffer[..width] {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
        remaining -= width as u64;
    }
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{durability::Journal, test_support::Fixture};
    use std::{
        io::{BufWriter, Write},
        time::Instant,
    };
    #[test]
    fn mixed_legacy_and_utc_rows_preserve_history_and_separate_paths_across_midnight() {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_writer(Vec::new());
        writer
            .write_record(["2026-10-14 23:59:59", "app.exe", "0.5"])
            .unwrap();
        let first = crate::history::identity(Path::new(r"C:\Uno\app.exe"));
        let second = crate::history::identity(Path::new(r"C:\Dos\app.exe"));
        writer
            .write_record([
                "2026-10-15T02:59:59.500000Z",
                "app.exe",
                "1.25",
                &first,
                r"C:\Uno\app.exe",
                "-10800",
            ])
            .unwrap();
        writer
            .write_record([
                "2026-10-15T03:00:00Z",
                "app.exe",
                "0.25",
                &second,
                r"C:\Dos\app.exe",
                "-10800",
            ])
            .unwrap();
        let bytes = writer.into_inner().unwrap();
        let f = Fixture::new("");
        fs::write(&f.csv, &bytes).unwrap();
        let s = DailySummary::load(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-10-14", "app.exe"), 0.5);
        assert_eq!(total(&s, "2026-10-14", &first), 0.5);
        assert_eq!(total(&s, "2026-10-15", &first), 0.75);
        assert_eq!(total(&s, "2026-10-15", &second), 0.25);
        let mut cache: serde_json::Value =
            serde_json::from_slice(&fs::read(cache_path(&f.csv)).unwrap()).unwrap();
        cache["version"] = 2.into();
        fs::write(cache_path(&f.csv), serde_json::to_vec(&cache).unwrap()).unwrap();
        let mut rebuilt = DailySummary::load(&f.csv).unwrap();
        rebuilt.sync(&f.csv).unwrap();
        assert_eq!(total(&rebuilt, "2026-10-15", &first), 0.75);
        assert_eq!(fs::read(&f.csv).unwrap(), bytes);
    }

    #[test]
    fn repeated_dst_hour_adds_both_elapsed_segments_once() {
        let f=Fixture::new("2026-11-01T05:30:00Z,app.exe,60,app.exe,,-14400\n2026-11-01T06:30:00Z,app.exe,60,app.exe,,-18000\n");
        let s = DailySummary::load(&f.csv).unwrap();
        assert_eq!(s.day(date("2026-11-01"))["app.exe"][1], 120.0);
        assert_eq!(
            DailySummary::load(&f.csv).unwrap().day(date("2026-11-01"))["app.exe"][1],
            120.0
        );
    }
    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }
    fn total(summary: &DailySummary, day: &str, app: &str) -> f64 {
        summary
            .day(date(day))
            .get(app)
            .map(|hours| hours.iter().sum())
            .unwrap_or(0.0)
    }

    #[test]
    fn splits_midnight_and_fractional_seconds() {
        let f =
            Fixture::new("2026-09-27 23:59:59,game.exe,2.25\n2026-09-28 01:59:59,game.exe,2.5\n");
        let s = DailySummary::load(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 1.0);
        let hours = s.day(date("2026-09-28"))["game.exe"];
        assert_eq!((hours[0], hours[1], hours[2]), (1.25, 1.0, 1.5));
    }

    #[test]
    fn restart_and_repeated_sync_do_not_duplicate_rows() {
        let f = Fixture::new("2026-09-27 10:00:00,game.exe,60\n");
        DailySummary::load(&f.csv).unwrap();
        Journal::open(&f.csv)
            .unwrap()
            .append(b"2026-09-27 11:00:00,game.exe,30.5\n")
            .unwrap();
        let mut s = DailySummary::load(&f.csv).unwrap();
        s.sync(&f.csv).unwrap();
        s.sync(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 90.5);
    }

    #[test]
    fn corrupt_cache_and_truncated_history_are_rebuilt() {
        let f = Fixture::new("2026-09-27 10:00:00,game.exe,60\n");
        fs::write(cache_path(&f.csv), b"broken json").unwrap();
        let mut s = DailySummary::load(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 60.0);
        fs::write(&f.csv, "2026-09-28 10:00:00,new.exe,5\n").unwrap();
        s.sync(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 0.0);
        assert_eq!(total(&s, "2026-09-28", "new.exe"), 5.0);
    }

    #[test]
    fn detects_middle_edit_even_with_new_rows_appended() {
        let row = "2026-09-27 10:00:00,game.exe,10\n";
        let f = Fixture::new(&row.repeat(1000));
        let mut s = DailySummary::load(&f.csv).unwrap();
        let mut content = row.repeat(1000);
        let offset = row.len() * 500;
        content.replace_range(
            offset..offset + row.len(),
            "2026-09-27 10:00:00,game.exe,20\n",
        );
        content.push_str(row);
        fs::write(&f.csv, content).unwrap();
        s.sync(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 10020.0);
        assert_eq!(
            total(
                &DailySummary::load(&f.csv).unwrap(),
                "2026-09-27",
                "game.exe"
            ),
            10020.0
        );
    }

    #[test]
    fn incomplete_row_does_not_poison_cache_and_recovers() {
        let f = Fixture::new("2026-09-27 10:00:00,game.exe,60\n");
        let mut s = DailySummary::load(&f.csv).unwrap();
        let before = fs::read(cache_path(&f.csv)).unwrap();
        fs::write(&f.csv, "2026-09-27 10:00:00,game.exe,60\n2026-09-27 11:").unwrap();
        assert!(s.sync(&f.csv).is_err());
        assert_eq!(before, fs::read(cache_path(&f.csv)).unwrap());
        fs::write(
            &f.csv,
            "2026-09-27 10:00:00,game.exe,60\n2026-09-27 11:00:00,game.exe,30\n",
        )
        .unwrap();
        s.sync(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 90.0);
    }

    #[test]
    fn trusted_append_hash_matches_full_file_and_external_edit_rebuilds() {
        let f = Fixture::new("2026-09-27 10:00:00,game.exe,60\n");
        let mut s = DailySummary::load(&f.csv).unwrap();
        let row = b"2026-09-27 11:00:00,game.exe,30\n";
        Journal::open(&f.csv).unwrap().append(row).unwrap();
        s.note_committed_append(row.len());
        s.sync(&f.csv).unwrap();
        let length = fs::metadata(&f.csv).unwrap().len();
        assert_eq!(
            s.source_hash,
            fingerprint(&mut File::open(&f.csv).unwrap(), length).unwrap()
        );
        assert_eq!(s.trusted_growth, 0);
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 90.0);

        let changed = "2026-09-27 10:00:00,game.exe,90\n2026-09-27 11:00:00,game.exe,30\n";
        fs::write(&f.csv, changed).unwrap();
        s.sync(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 120.0);
    }

    #[test]
    fn unreasonable_csv_duration_does_not_expand_into_many_days() {
        let f = Fixture::new("2026-09-27 10:00:00,game.exe,1e200\n");
        let s = DailySummary::load(&f.csv).unwrap();
        assert_eq!(total(&s, "2026-09-27", "game.exe"), 0.0);
    }

    #[test]
    #[ignore = "Run manually in Release to measure synthetic multi-year histories"]
    fn benchmark_large_histories() {
        let first = date("2020-01-01");
        for years in [1, 3, 5] {
            let fixture = Fixture::new("");
            let mut writer = BufWriter::new(File::create(&fixture.csv).unwrap());
            let days = years * 365;
            for day in 0..days {
                let date = first + chrono::Duration::days(day as i64);
                for interval in 0..288 {
                    let minutes = interval * 5;
                    writeln!(
                        writer,
                        "{} {:02}:{:02}:00,app{:02}.exe,60",
                        date,
                        minutes / 60,
                        minutes % 60,
                        interval % 24
                    )
                    .unwrap();
                }
            }
            writer.flush().unwrap();
            drop(writer);
            let csv_bytes = fs::metadata(&fixture.csv).unwrap().len();

            let started = Instant::now();
            let cold_summary = DailySummary::load(&fixture.csv).unwrap();
            let cold = started.elapsed();
            assert_eq!(cold_summary.source_len, csv_bytes);
            drop(cold_summary);
            let cache_bytes = fs::metadata(cache_path(&fixture.csv)).unwrap().len();
            let started = Instant::now();
            let mut summary = DailySummary::load(&fixture.csv).unwrap();
            let reopen = started.elapsed();

            let last_date = first + chrono::Duration::days(days as i64 - 1);
            let mut append_times = Vec::new();
            for minute in 0..5 {
                let row = format!("{last_date} 23:59:{minute:02},bench.exe,1\n");
                Journal::open(&fixture.csv)
                    .unwrap()
                    .append(row.as_bytes())
                    .unwrap();
                summary.note_committed_append(row.len());
                let started = Instant::now();
                summary.sync(&fixture.csv).unwrap();
                append_times.push(started.elapsed());
            }
            assert_eq!(total(&summary, &last_date.to_string(), "bench.exe"), 5.0);
            assert_eq!(
                summary.source_len,
                fs::metadata(&fixture.csv).unwrap().len()
            );
            let append_mean_ms = append_times
                .iter()
                .map(|time| time.as_secs_f64())
                .sum::<f64>()
                * 1000.0
                / append_times.len() as f64;
            println!(
                "years={years} rows={} csv_mb={:.2} cache_mb={:.2} cold_ms={:.1} reopen_ms={:.1} append_mean_ms={append_mean_ms:.1}",
                days * 288,
                csv_bytes as f64 / 1048576.0,
                cache_bytes as f64 / 1048576.0,
                cold.as_secs_f64() * 1000.0,
                reopen.as_secs_f64() * 1000.0,
            );
        }
    }

    #[test]
    #[ignore = "Run manually in Release; generates one year of one-minute checkpoint rows"]
    fn benchmark_checkpoint_cadence() {
        let fixture = Fixture::new("");
        let mut writer = BufWriter::new(File::create(&fixture.csv).unwrap());
        let first = date("2025-01-01");
        for day in 0..365 {
            let date = first + chrono::Duration::days(day);
            for interval in 0..1440 {
                let seconds = interval * 60;
                writeln!(
                    writer,
                    "{} {:02}:{:02}:{:02},app{:02}.exe,60",
                    date,
                    seconds / 3600,
                    (seconds % 3600) / 60,
                    seconds % 60,
                    interval % 24
                )
                .unwrap();
            }
        }
        writer.flush().unwrap();
        drop(writer);
        let csv_bytes = fs::metadata(&fixture.csv).unwrap().len();

        let started = Instant::now();
        let cold = DailySummary::load(&fixture.csv).unwrap();
        let cold_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(cold.source_len, csv_bytes);
        drop(cold);
        let cache_bytes = fs::metadata(cache_path(&fixture.csv)).unwrap().len();

        let started = Instant::now();
        let mut warm = DailySummary::load(&fixture.csv).unwrap();
        let reopen_ms = started.elapsed().as_secs_f64() * 1000.0;
        let row = b"2025-12-31 23:59:59,bench.exe,1\n";
        Journal::open(&fixture.csv).unwrap().append(row).unwrap();
        warm.note_committed_append(row.len());
        let started = Instant::now();
        warm.sync(&fixture.csv).unwrap();
        let append_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(total(&warm, "2025-12-31", "bench.exe"), 1.0);
        println!(
            "checkpoint_year rows=525600 csv_mb={:.2} cache_mb={:.2} cold_ms={cold_ms:.1} reopen_ms={reopen_ms:.1} append_ms={append_ms:.1}",
            csv_bytes as f64 / 1048576.0,
            cache_bytes as f64 / 1048576.0
        );
    }
}
