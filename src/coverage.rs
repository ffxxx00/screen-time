use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Default, Serialize, Deserialize)]
pub struct Coverage {
    days: BTreeSet<NaiveDate>,
    #[serde(default, rename = "startDate")]
    start_date: Option<NaiveDate>,
    #[serde(skip)]
    path: PathBuf,
    #[serde(skip)]
    writable: bool,
    #[serde(skip)]
    needs_save: bool,
}

impl Coverage {
    fn csv_created(csv: &Path) -> Option<NaiveDate> {
        fs::metadata(csv)
            .ok()?
            .created()
            .ok()
            .map(|at| DateTime::<Local>::from(at).date_naive())
    }

    pub fn load(csv: &Path) -> Result<Self, String> {
        let path = csv.with_extension("coverage.json");
        let mut coverage = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes)
                .map_err(|error| format!("No se pudo leer {}: {error}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(format!("No se pudo leer {}: {error}", path.display())),
        };
        coverage.path = path;
        coverage.writable = true;
        if coverage.start_date.is_none() {
            coverage.start_date = Self::csv_created(csv).or_else(|| coverage.days.first().copied());
            coverage.needs_save = coverage.start_date.is_some();
        }
        Ok(coverage)
    }

    pub fn unavailable(csv: &Path) -> Self {
        Self {
            days: BTreeSet::new(),
            start_date: Self::csv_created(csv),
            path: csv.with_extension("coverage.json"),
            writable: false,
            needs_save: false,
        }
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        self.days.contains(&date)
    }
    pub fn first_date(&self) -> Option<NaiveDate> {
        self.start_date
            .into_iter()
            .chain(self.days.first().copied())
            .min()
    }
    pub fn is_available(&self) -> bool {
        self.writable
    }

    pub fn mark(&mut self, date: NaiveDate) -> Result<(), String> {
        if !self.writable {
            return Err("El archivo de cobertura no está disponible".into());
        }
        if self.days.contains(&date) && !self.needs_save {
            return Ok(());
        }
        let inserted = self.days.insert(date);
        let result = serde_json::to_vec(&self)
            .map_err(|error| error.to_string())
            .and_then(|bytes| crate::durability::atomic_write(&self.path, &bytes));
        if result.is_err() && inserted {
            self.days.remove(&date);
        }
        if result.is_ok() {
            self.needs_save = false;
        }
        result
    }
}
