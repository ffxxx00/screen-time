use chrono::Local;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

const MAX_LOG_BYTES: u64 = 64 * 1024;

pub fn record(event: &str) {
    let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return;
    };
    let directory = base.join("Screen Time");
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join("lifecycle.log");
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() >= MAX_LOG_BYTES) {
        let _ = fs::remove_file(directory.join("lifecycle.previous.log"));
        let _ = fs::rename(&path, directory.join("lifecycle.previous.log"));
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "{} pid={} {}",
            Local::now().to_rfc3339(),
            std::process::id(),
            event
        );
        let _ = file.sync_data();
    }
}
