//! Display-only compatibility between legacy executable names and precise identities.
use crate::{app_identity, history};
use std::{
    cell::OnceCell,
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
#[derive(Default)]
pub struct Grouping {
    paths: BTreeSet<String>,
    windows: BTreeMap<String, BTreeSet<String>>,
    native: BTreeMap<String, BTreeSet<String>>,
    installations: BTreeMap<String, BTreeSet<String>>,
    aliases: OnceCell<BTreeMap<String, String>>,
}
pub struct Presentation<'a> {
    pub name: &'a str,
    pub icon: Option<&'a str>,
}
impl Grouping {
    pub fn load(csv: &Path) -> Self {
        let mut result = Self::default();
        if let Ok(mut reader) = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(csv)
        {
            for row in reader.records().flatten() {
                if row.len() >= 6 && history::parse(&row).is_some() && !row[4].is_empty() {
                    result.observe(&row[3], Path::new(&row[4]));
                }
            }
        }
        result
    }
    pub fn observe(&mut self, id: &str, path: &Path) {
        if app_identity::is_host(path) {
            return;
        }
        let path = history::identity(path);
        let new_path = self.paths.insert(path.clone());
        let mut new_identity = false;
        if id.starts_with("app:windows:") {
            new_identity = self.windows.entry(id.into()).or_default().insert(path);
        }
        if new_path || new_identity {
            self.aliases.take();
        }
    }
    pub fn catalog(&mut self, catalog: &BTreeMap<String, app_identity::Metadata>) {
        self.aliases.take();
        self.native.clear();
        self.installations.clear();
        for (id, metadata) in catalog {
            if !id.starts_with("app:native:") {
                continue;
            }
            if let Some(native) = &metadata.native {
                for path in &native.paths {
                    if let Some(key) = crate::native_identity::installation_key(path) {
                        self.installations
                            .entry(key)
                            .or_default()
                            .insert(id.clone());
                    }
                    self.paths.insert(path.clone());
                    self.native
                        .entry(path.clone())
                        .or_default()
                        .insert(id.clone());
                }
            }
        }
    }
    fn aliases(&self) -> &BTreeMap<String, String> {
        self.aliases.get_or_init(|| self.build_aliases())
    }

    fn build_aliases(&self) -> BTreeMap<String, String> {
        let mut aliases = BTreeMap::new();
        let mut installations = self.installations.clone();
        // Path-only rows must benefit as well, including executables that
        // cannot be fingerprinted and older six-column CSV records.
        for path in &self.paths {
            if let Some(key) = crate::native_identity::installation_key(path) {
                installations.entry(key).or_default().insert(path.clone());
            }
        }
        // An executable can have visited more than one installation root.
        // Resolve connected groups before assigning path and legacy aliases.
        for ids in installations.values() {
            let canonical = ids
                .iter()
                .map(|id| aliases.get(id).unwrap_or(id))
                .min()
                .unwrap()
                .clone();
            let previous: BTreeSet<_> = ids
                .iter()
                .map(|id| aliases.get(id).unwrap_or(id).clone())
                .collect();
            for value in aliases.values_mut() {
                if previous.contains(value) {
                    *value = canonical.clone();
                }
            }
            for id in ids {
                aliases.insert(id.clone(), canonical.clone());
            }
        }
        for (path, identities) in &self.native {
            if identities.len() == 1 {
                let id = identities.first().unwrap();
                aliases.insert(path.clone(), aliases.get(id).unwrap_or(id).clone());
            }
        }
        let mut names = BTreeMap::<String, BTreeSet<String>>::new();
        for path in &self.paths {
            names
                .entry(history::executable_name(path).to_lowercase())
                .or_default()
                .insert(aliases.get(path).unwrap_or(path).clone());
        }
        for (id, paths) in &self.windows {
            let candidates: BTreeSet<_> = paths
                .iter()
                .map(|path| aliases.get(path).unwrap_or(path).clone())
                .collect();
            if candidates.len() == 1 {
                aliases.insert(id.clone(), candidates.first().unwrap().clone());
            }
        }
        for (name, paths) in names {
            if paths.len() == 1 {
                aliases.insert(name, paths.first().unwrap().clone());
            }
        }
        aliases
    }
    pub fn presentation<'a>(
        &self,
        catalog: &'a BTreeMap<String, app_identity::Metadata>,
    ) -> BTreeMap<String, Presentation<'a>> {
        let aliases = self.aliases();
        let mut result = BTreeMap::<String, Presentation<'a>>::new();
        for (id, metadata) in catalog {
            // Native window AUMIDs can describe a browser profile or document.
            // Their labels must not replace the native application's own name.
            if id.starts_with("app:windows:")
                && aliases.get(id).is_some_and(|canonical| canonical != id)
            {
                continue;
            }
            let id = aliases.get(id).unwrap_or(id).clone();
            let current = result.entry(id).or_insert(Presentation {
                name: &metadata.name,
                icon: None,
            });
            if current.name.is_empty()
                || (current.name.to_ascii_lowercase().ends_with(".exe")
                    && !metadata.name.to_ascii_lowercase().ends_with(".exe"))
            {
                current.name = &metadata.name;
            }
            if current.icon.is_none() {
                current.icon = metadata.icon.as_deref();
            }
        }
        result
    }
    pub fn group<const N: usize>(
        &self,
        usage: BTreeMap<String, [f64; N]>,
    ) -> BTreeMap<String, [f64; N]> {
        let aliases = self.aliases();
        let mut result = BTreeMap::<String, [f64; N]>::new();
        for (id, values) in usage {
            let lookup = if id.starts_with("app:") {
                id.clone()
            } else {
                id.to_lowercase()
            };
            let canonical = aliases.get(&lookup).cloned().unwrap_or(id);
            let total = result.entry(canonical).or_insert([0.0; N]);
            for (total, value) in total.iter_mut().zip(values) {
                *total += value;
            }
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versioned_paths_group_without_hash_catalog_or_existing_files() {
        let mut grouping = Grouping::default();
        let first = Path::new(r"C:\Tools\app-1.0.0\bin\tool.exe");
        let second = Path::new(r"C:\Tools\app-2.0.0\bin\tool.exe");
        for path in [first, second] {
            grouping.observe(&history::identity(path), path);
        }
        let totals = grouping.group(BTreeMap::from([
            (history::identity(first), [30.0, 10.0]),
            (history::identity(second), [20.0, 40.0]),
        ]));
        assert_eq!(totals.len(), 1);
        assert_eq!(totals.values().next().unwrap(), &[50.0, 50.0]);
    }
    #[test]
    fn updated_metadata_enriches_the_entire_group_without_copying_or_merging_catalog() {
        let make = |path: &str, name: &str, icon: Option<&str>| app_identity::Metadata {
            name: name.into(),
            icon: icon.map(str::to_owned),
            native: Some(crate::native_identity::Evidence {
                paths: BTreeSet::from([path.into()]),
                ..Default::default()
            }),
        };
        let catalog = BTreeMap::from([
            (
                "app:native:a".into(),
                make(
                    r"path:c:\tools\app-1.0.0\bin\editor.exe",
                    "editor.exe",
                    None,
                ),
            ),
            (
                "app:native:b".into(),
                make(
                    r"path:c:\tools\app-2.0.0\bin\editor.exe",
                    "Example Workshop",
                    Some("synthetic-icon"),
                ),
            ),
        ]);
        let mut grouping = Grouping::default();
        grouping.catalog(&catalog);
        let views = grouping.presentation(&catalog);
        assert_eq!(views.len(), 1);
        assert_eq!(views["app:native:a"].name, "Example Workshop");
        assert_eq!(views["app:native:a"].icon, Some("synthetic-icon"));
        assert_eq!(catalog.len(), 2);
    }
    #[test]
    fn legacy_path_and_profile_sum_without_rewriting_history() {
        let mut grouping = Grouping::default();
        let path = Path::new(r"C:\Apps\browser.exe");
        let id = history::identity(path);
        grouping.observe(&id, path);
        grouping.observe("app:windows:Browser.Profile1", path);
        grouping.observe("app:windows:Browser.Profile2", path);
        let grouped = grouping.group(BTreeMap::from([
            ("Browser.exe".into(), [1.0, 2.0]),
            (id.clone(), [3.0, 4.0]),
            ("app:windows:Browser.Profile1".into(), [5.0, 6.0]),
            ("app:windows:Browser.Profile2".into(), [7.0, 8.0]),
        ]));
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[&id], [16.0, 20.0]);
    }
    #[test]
    fn same_named_apps_and_hosted_apps_are_not_merged() {
        let mut grouping = Grouping::default();
        for path in [r"C:\One\app.exe", r"C:\Two\app.exe", r"C:\Java\javaw.exe"] {
            grouping.observe(&history::identity(Path::new(path)), Path::new(path));
        }
        grouping.observe("app:windows:Hosted.One", Path::new(r"C:\Java\javaw.exe"));
        let ids = [
            "app.exe",
            "path:c:\\one\\app.exe",
            "path:c:\\two\\app.exe",
            "javaw.exe",
            "path:c:\\java\\javaw.exe",
            "app:windows:Hosted.One",
            "app:java:class:org.example.Two",
        ];
        let usage = ids.iter().map(|id| (id.to_string(), [1.0])).collect();
        assert_eq!(grouping.group(usage).len(), ids.len());
    }
}
