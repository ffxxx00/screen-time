use std::{
    fs,
    path::{Path, PathBuf},
};

// A durable intent makes an interrupted deletion finish before journal recovery.
// Only known history files are affected; preferences and exports are retained.
pub fn finish_reset(csv: &Path) -> Result<(), String> {
    let marker = csv.with_extension("reset.json");
    match fs::metadata(&marker) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
        Ok(_) => {}
    }
    for extension in [
        "pending.json",
        "daily.json",
        "coverage.json",
        "paths.json",
        "pending.json.tmp",
        "daily.json.tmp",
        "coverage.json.tmp",
        "paths.json.tmp",
    ] {
        match fs::remove_file(csv.with_extension(extension)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    crate::durability::atomic_write(csv, b"")?;
    fs::remove_file(marker).map_err(|error| error.to_string())
}

pub fn reset(csv: &Path) -> Result<(), String> {
    crate::durability::atomic_write(&csv.with_extension("reset.json"), b"{}")?;
    finish_reset(csv)
}

pub fn export(csv: &Path) -> Result<PathBuf, String> {
    let folder = csv
        .parent()
        .ok_or("No se encontró la carpeta de datos")?
        .join("Exportaciones")
        .join(chrono::Local::now().format("%Y%m%d-%H%M%S-%f").to_string());
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    fs::copy(csv, folder.join("app_usage.csv")).map_err(|error| error.to_string())?;
    fs::write(folder.join("LEEME.txt"), "Historial local de Screen Time. CSV sin cabecera. Filas nuevas: instante UTC RFC3339, ejecutable, segundos (con fracciones), identidad por ruta, ruta y desfase UTC en segundos. Filas antiguas: fecha y hora local sin zona, ejecutable y segundos. Las filas antiguas se conservan sin inferir su zona ni ruta. Los días sin registros no certifican cero uso. Esta copia no se elimina al borrar el historial desde la aplicación.\r\n").map_err(|error| error.to_string())?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    #[test]
    fn interrupted_reset_finishes_before_pending_rows_can_reappear() {
        let fixture = Fixture::new("reset");
        let csv = fixture.csv.clone();
        fs::write(&csv, "old history").unwrap();
        for extension in [
            "pending.json",
            "daily.json",
            "coverage.json",
            "paths.json",
            "settings.json",
        ] {
            fs::write(csv.with_extension(extension), "{}").unwrap();
        }
        fs::write(csv.with_extension("reset.json"), "{}").unwrap();
        finish_reset(&csv).unwrap();
        assert_eq!(fs::read(&csv).unwrap(), b"");
        assert!(csv.with_extension("settings.json").exists());
        assert!(!csv.with_extension("pending.json").exists());
        assert!(!csv.with_extension("coverage.json").exists());
        finish_reset(&csv).unwrap();
    }

    #[test]
    fn failed_reset_keeps_intent_and_can_be_retried() {
        let fixture = Fixture::new("synthetic history");
        let csv = fixture.csv.clone();
        fs::create_dir(csv.with_extension("coverage.json")).unwrap();
        assert!(reset(&csv).is_err());
        assert!(csv.with_extension("reset.json").exists());
        assert_eq!(fs::read(&csv).unwrap(), b"synthetic history");
        fs::remove_dir(csv.with_extension("coverage.json")).unwrap();
        finish_reset(&csv).unwrap();
        assert!(fs::read(&csv).unwrap().is_empty());
        assert!(!csv.with_extension("reset.json").exists());
    }

    #[test]
    fn export_preserves_fractional_csv_and_survives_reset() {
        let fixture = Fixture::new("export");
        let csv = fixture.csv.clone();
        let bytes = b"2026-09-30 12:00:00,synthetic.exe,0.5\n";
        fs::write(&csv, bytes).unwrap();
        let folder = export(&csv).unwrap();
        reset(&csv).unwrap();
        assert_eq!(fs::read(folder.join("app_usage.csv")).unwrap(), bytes);
    }
}
