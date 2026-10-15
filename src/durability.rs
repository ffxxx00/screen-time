use base64::Engine;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension().unwrap_or_default().to_string_lossy()
    ));
    let mut file = File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    let wide = |p: &Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>()
    };
    let from = wide(&temp);
    let to = wide(path);
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

pub struct Journal {
    csv: PathBuf,
    pending: PathBuf,
    active: Option<(u64, Vec<u8>)>,
}

#[cfg(test)]
fn crash_for_test(stage: &str) {
    if std::env::var("SCREEN_TIME_TEST_CRASH_STAGE").as_deref() == Ok(stage) {
        // The child process exits without unwinding or dropping Journal/File.
        // The parent test opens the same synthetic history after termination.
        std::process::exit(77);
    }
}

impl Journal {
    pub fn open(csv: &Path) -> Result<Self, String> {
        let pending = csv.with_extension("pending.json");
        let active = match fs::read(&pending) {
            Ok(bytes) => {
                let data: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Registro de recuperación inválido: {e}"))?;
                let offset = data["offset"]
                    .as_u64()
                    .ok_or("Falta offset en recuperación")?;
                let row = base64::engine::general_purpose::STANDARD
                    .decode(data["row"].as_str().ok_or("Falta fila en recuperación")?)
                    .map_err(|e| e.to_string())?;
                Some((offset, row))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let mut journal = Self {
            csv: csv.into(),
            pending,
            active,
        };
        if journal.active.is_some() {
            journal.finish()?;
        }
        Ok(journal)
    }
    pub fn append(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.active.is_none() {
            self.active = Some((
                fs::metadata(&self.csv).map_err(|e| e.to_string())?.len(),
                bytes.to_vec(),
            ));
        }
        #[cfg(test)]
        crash_for_test("before_journal");
        let (offset, row) = self.active.as_ref().unwrap();
        let data = serde_json::json!({"offset":offset,"row":base64::engine::general_purpose::STANDARD.encode(row)});
        atomic_write(&self.pending, data.to_string().as_bytes())?;
        #[cfg(test)]
        crash_for_test("after_journal");
        self.finish()
    }
    fn finish(&mut self) -> Result<(), String> {
        let (offset, row) = self.active.as_ref().unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.csv)
            .map_err(|e| e.to_string())?;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        if length < *offset {
            return Err(
                "El historial fue truncado; se conserva el registro de recuperación.".into(),
            );
        }
        file.seek(SeekFrom::Start(*offset))
            .map_err(|e| e.to_string())?;
        let available = ((length - offset) as usize).min(row.len());
        let mut existing = vec![0; available];
        file.read_exact(&mut existing).map_err(|e| e.to_string())?;
        if existing != row[..available] {
            return Err("El historial no coincide con la recuperación; no se modificó.".into());
        }
        #[cfg(test)]
        if std::env::var("SCREEN_TIME_TEST_CRASH_STAGE").as_deref() == Ok("partial_csv") {
            let remaining = &row[available..];
            file.write_all(&remaining[..remaining.len() / 2])
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            crash_for_test("partial_csv");
        }
        // A previous attempt may already have written all or part of this row.
        file.write_all(&row[available..])
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        #[cfg(test)]
        crash_for_test("after_csv_sync");
        self.active = None;
        // If deletion fails, replay is still idempotent; a later append replaces it.
        let _ = fs::remove_file(&self.pending);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;
    use std::process::Command;

    const CRASH_ROW: &[u8] = b"2026-09-28 10:00:00,game.exe,12.5\n";

    #[test]
    fn crash_worker() {
        let Ok(path) = std::env::var("SCREEN_TIME_TEST_CRASH_CSV") else {
            return;
        };
        Journal::open(Path::new(&path))
            .unwrap()
            .append(CRASH_ROW)
            .unwrap();
        panic!("El punto de terminación no fue alcanzado");
    }

    #[test]
    fn real_process_exit_during_checkpoint_recovers_once() {
        for stage in [
            "before_journal",
            "after_journal",
            "partial_csv",
            "after_csv_sync",
        ] {
            let f = Fixture::new("2026-09-27 10:00:00,game.exe,60\n");
            let original = fs::read(&f.csv).unwrap();
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("durability::tests::crash_worker")
                .env("SCREEN_TIME_TEST_CRASH_CSV", &f.csv)
                .env("SCREEN_TIME_TEST_CRASH_STAGE", stage)
                .output()
                .unwrap();
            assert_eq!(
                child.status.code(),
                Some(77),
                "stage: {stage}; stderr: {}",
                String::from_utf8_lossy(&child.stderr)
            );
            Journal::open(&f.csv).unwrap();
            Journal::open(&f.csv).unwrap();
            let mut expected = original;
            if stage != "before_journal" {
                expected.extend_from_slice(CRASH_ROW);
            }
            assert_eq!(fs::read(&f.csv).unwrap(), expected, "stage: {stage}");
            assert!(!f.csv.with_extension("pending.json").exists());
            Journal::open(&f.csv)
                .unwrap()
                .append(b"2026-09-28 10:01:00,game.exe,1\n")
                .unwrap();
            assert_eq!(
                fs::read(&f.csv)
                    .unwrap()
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count(),
                if stage == "before_journal" { 2 } else { 3 }
            );
        }
    }
    #[test]
    fn interrupted_append_recovers_every_partial_prefix_once() {
        let row = b"2026-09-28 10:00:00,game.exe,12.5\n";
        for prefix in 0..=row.len() {
            let f = Fixture::new("2026-09-27 10:00:00,game.exe,60\n");
            let mut expected = fs::read(&f.csv).unwrap();
            let offset = expected.len();
            let pending = serde_json::json!({"offset":offset, "row":base64::engine::general_purpose::STANDARD.encode(row)});
            fs::write(f.csv.with_extension("pending.json"), pending.to_string()).unwrap();
            let mut partial = expected.clone();
            partial.extend_from_slice(&row[..prefix]);
            fs::write(&f.csv, partial).unwrap();
            Journal::open(&f.csv).unwrap();
            Journal::open(&f.csv).unwrap();
            expected.extend_from_slice(row);
            assert_eq!(fs::read(&f.csv).unwrap(), expected);
        }
    }
    #[test]
    fn recovery_conflict_preserves_history_and_journal() {
        let f = Fixture::new("different content\n");
        let before = fs::read(&f.csv).unwrap();
        let pending = f.csv.with_extension("pending.json");
        fs::write(&pending, serde_json::json!({"offset":0,"row":base64::engine::general_purpose::STANDARD.encode(b"expected\n")}).to_string()).unwrap();
        assert!(Journal::open(&f.csv).is_err());
        assert_eq!(fs::read(&f.csv).unwrap(), before);
        assert!(pending.exists());
    }
}
