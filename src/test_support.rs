use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Fixture {
    pub root: PathBuf,
    pub csv: PathBuf,
}
impl Fixture {
    pub fn new(content: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "screen-time-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let csv = root.join("app_usage.csv");
        fs::write(&csv, content).unwrap();
        Self { root, csv }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
