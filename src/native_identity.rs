//! Content evidence for relocations, computed only on the identity worker.
use crate::{app_identity, history};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::Path,
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub fingerprints: BTreeSet<String>,
    pub paths: BTreeSet<String>,
}

type Cache = BTreeMap<String, (u64, SystemTime, Option<String>)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

/// Recognize version directories used by MSIX and Squirrel. Keep the install
/// root, package publisher, architecture and executable relative path intact.
pub fn installation_key(path: &str) -> Option<String> {
    let normalized = path.to_ascii_lowercase().replace('/', "\\");
    let mut parts: Vec<_> = normalized.split('\\').map(str::to_owned).collect();
    if let Some(index) = parts.iter().position(|part| part == "windowsapps") {
        let package = parts.get(index + 1)?;
        let fields: Vec<_> = package.split('_').collect();
        if fields.len() == 5 && numeric_version(fields[1]) && !fields[4].is_empty() {
            parts[index + 1] = format!("{}_{}_{}_{}", fields[0], fields[2], fields[3], fields[4]);
            return Some(parts.join("\\"));
        }
    }
    if parts.len() >= 3 && parts.last()?.ends_with(".exe") {
        // Squirrel applications need not name the installation directory after
        // the executable, and may keep executables in nested subdirectories.
        for index in 1..parts.len() - 1 {
            if !parts[index]
                .strip_prefix("app-")
                .is_some_and(numeric_version)
            {
                continue;
            }
            parts.remove(index);
            return Some(parts.join("\\"));
        }
    }
    None
}
fn numeric_version(value: &str) -> bool {
    let fields: Vec<_> = value.split('.').collect();
    fields.len() >= 3
        && fields
            .iter()
            .all(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
}

pub fn same_installation(one: &Evidence, two: &Evidence) -> bool {
    let keys: BTreeSet<_> = one
        .paths
        .iter()
        .filter_map(|p| installation_key(p))
        .collect();
    two.paths
        .iter()
        .filter_map(|p| installation_key(p))
        .any(|key| keys.contains(&key))
}

// A renamed shared runtime must not become a native application merely because
// its filename changed. Version strings are exclusion hints, never merge proof.
pub fn version_strings(path: &Path) -> BTreeMap<String, String> {
    use windows_sys::Win32::Storage::FileSystem::*;
    let mut result = BTreeMap::new();
    unsafe {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), std::ptr::null_mut());
        if size == 0 || size > 1024 * 1024 {
            return result;
        }
        let mut bytes = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, bytes.as_mut_ptr().cast()) == 0 {
            return result;
        }
        let query: Vec<u16> = "\\VarFileInfo\\Translation"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut value = std::ptr::null_mut();
        let mut length = 0;
        if VerQueryValueW(
            bytes.as_ptr().cast(),
            query.as_ptr(),
            &mut value,
            &mut length,
        ) == 0
            || length < 4
        {
            return result;
        }
        let translations =
            std::slice::from_raw_parts(value.cast::<u16>(), length as usize / 2).to_vec();
        for pair in translations.chunks_exact(2) {
            for field in ["OriginalFilename", "FileDescription", "ProductName"] {
                let query: Vec<u16> =
                    format!("\\StringFileInfo\\{:04x}{:04x}\\{field}", pair[0], pair[1])
                        .encode_utf16()
                        .chain(Some(0))
                        .collect();
                if VerQueryValueW(
                    bytes.as_ptr().cast(),
                    query.as_ptr(),
                    &mut value,
                    &mut length,
                ) != 0
                    && length > 1
                {
                    let text = String::from_utf16_lossy(std::slice::from_raw_parts(
                        value.cast::<u16>(),
                        length as usize - 1,
                    ));
                    let text = text.trim();
                    if !text.is_empty() && text.len() <= 512 && !text.chars().any(char::is_control)
                    {
                        result.entry(field.into()).or_insert_with(|| text.into());
                    }
                }
            }
        }
    }
    result
}
fn shared_runtime(path: &Path) -> bool {
    app_identity::is_host(path)
        || version_strings(path)
            .get("OriginalFilename")
            .is_some_and(|original| app_identity::is_host(Path::new(original)))
}

fn fingerprint(path: &Path) -> Option<String> {
    // Deny concurrent writers; allow the running executable and renames.
    let mut file = File::options()
        .read(true)
        .share_mode(1 | 4)
        .open(path)
        .ok()?;
    let before = file.metadata().ok()?;
    if !before.is_file() || before.len() == 0 || before.len() > 512 * 1024 * 1024 {
        return None;
    }
    let modified = before.modified().ok()?;
    let key = history::identity(path);
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some((size, at, digest)) = cache.get(&key) {
            if *size == before.len() && *at == modified {
                return digest.clone();
            }
        }
    }
    let digest = if shared_runtime(path) {
        None
    } else {
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let length = file.read(&mut buffer).ok()?;
            if length == 0 {
                break;
            }
            hash.update(&buffer[..length]);
        }
        let after = file.metadata().ok()?;
        if before.len() != after.len() || after.modified().ok()? != modified {
            return None;
        }
        Some(format!("{:x}", hash.finalize()))
    };
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(key, (before.len(), modified, digest.clone()));
    }
    digest
}

pub fn identify(path: &Path) -> Option<(String, Evidence)> {
    if app_identity::is_host(path) {
        return None;
    }
    let digest = fingerprint(path)?;
    Some((
        format!("app:native:{digest}"),
        Evidence {
            fingerprints: BTreeSet::from([digest]),
            paths: BTreeSet::from([history::identity(path)]),
        },
    ))
}

/// Reuse the established identity across moves and updates in a known location.
/// Conflicting locations are not sufficient evidence to merge two known apps.
pub fn reconcile(
    result: &mut app_identity::Resolved,
    catalog: &BTreeMap<String, app_identity::Metadata>,
) {
    let Some(evidence) = result.metadata.native.as_mut() else {
        return;
    };
    let exact: Vec<_> = catalog
        .iter()
        .filter(|(_, old)| {
            old.native
                .as_ref()
                .is_some_and(|old| !old.fingerprints.is_disjoint(&evidence.fingerprints))
        })
        .collect();
    let located: Vec<_> = catalog
        .iter()
        .filter(|(_, old)| {
            old.native.as_ref().is_some_and(|old| {
                !old.paths.is_disjoint(&evidence.paths) || same_installation(old, evidence)
            })
        })
        .collect();
    let selected = if exact.len() == 1 {
        exact.first()
    } else if exact.is_empty()
        && !located.is_empty()
        && located
            .iter()
            .all(|(_, old)| same_installation(old.native.as_ref().unwrap(), evidence))
    {
        located.first()
    } else if exact.is_empty() && located.len() == 1 {
        located.first()
    } else {
        None
    };
    if let Some((id, old)) = selected {
        result.id = (*id).clone();
        let old_evidence = old.native.as_ref().unwrap();
        evidence.paths.extend(old_evidence.paths.iter().cloned());
        evidence
            .fingerprints
            .extend(old_evidence.fingerprints.iter().cloned());
        if result.metadata.name.to_ascii_lowercase().ends_with(".exe") {
            result.metadata.name = old.name.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_description_comes_from_windows_version_resource() {
        let fields = version_strings(&std::env::current_exe().unwrap());
        assert_eq!(
            fields.get("FileDescription").map(String::as_str),
            Some("Screen Time")
        );
        assert!(version_strings(Path::new(r"C:\not-present-example\missing.exe")).is_empty());
    }
    #[test]
    fn update_keys_keep_publishers_installations_and_hosts_separate() {
        let key = installation_key;
        assert_eq!(
            key(r"path:c:\windowsapps\Example_1.2.3.4_x64__pub\app\tool.exe"),
            key(r"path:c:\windowsapps\Example_2.0.0.0_x64__pub\app\tool.exe")
        );
        assert_ne!(
            key(r"path:c:\windowsapps\Example_1.2.3.4_x64__pub\app\tool.exe"),
            key(r"path:c:\windowsapps\Example_1.2.3.4_x64__other\app\tool.exe")
        );
        assert_eq!(
            key(r"path:c:\local\Discord\app-1.0.1\Discord.exe"),
            key(r"path:c:\local\Discord\app-1.0.2\Discord.exe")
        );
        assert_ne!(
            key(r"path:c:\local\Discord\app-1.0.1\Discord.exe"),
            key(r"path:d:\local\Discord\app-1.0.2\Discord.exe")
        );
        assert!(key(r"path:c:\local\Discord\app-bad\Discord.exe").is_none());
        assert_ne!(
            key(r"path:c:\games\app-1.0.1\one.exe"),
            key(r"path:c:\games\app-1.0.1\two.exe")
        );
        assert_eq!(
            key(r"path:c:\tools\app-1.0.1\bin\editor.exe"),
            key(r"path:c:\tools\app-2.0.0\bin\editor.exe")
        );
    }
    #[test]
    fn new_version_reuses_existing_installation_identity() {
        let metadata = |version: &str, digest: &str| app_identity::Metadata {
            name: "chat.exe".into(),
            icon: None,
            native: Some(Evidence {
                fingerprints: BTreeSet::from([digest.into()]),
                paths: BTreeSet::from([format!(
                    r"path:c:\windowsapps\Example_{version}_x64__pub\app\chat.exe"
                )]),
            }),
        };
        let catalog = BTreeMap::from([("app:native:old".into(), metadata("1.0.0.0", "old"))]);
        let mut updated = app_identity::Resolved {
            id: "app:native:new".into(),
            metadata: metadata("2.0.0.0", "new"),
        };
        reconcile(&mut updated, &catalog);
        assert_eq!(updated.id, "app:native:old");
        assert_eq!(
            updated.metadata.native.as_ref().unwrap().fingerprints.len(),
            2
        );
        assert_eq!(updated.metadata.native.as_ref().unwrap().paths.len(), 2);
    }
    #[test]
    fn conflicting_known_apps_are_not_united_by_a_reused_path() {
        let make = |digest: &str, path: &str| app_identity::Metadata {
            name: "Same name".into(),
            icon: None,
            native: Some(Evidence {
                fingerprints: BTreeSet::from([digest.into()]),
                paths: BTreeSet::from([path.into()]),
            }),
        };
        let catalog = BTreeMap::from([
            ("app:native:one".into(), make("one", "path:c:\\app.exe")),
            ("app:native:two".into(), make("two", "path:d:\\app.exe")),
        ]);
        let mut moved = app_identity::Resolved {
            id: "app:native:two".into(),
            metadata: make("two", "path:c:\\app.exe"),
        };
        reconcile(&mut moved, &catalog);
        assert_eq!(moved.id, "app:native:two");
        assert_eq!(
            moved.metadata.native.as_ref().unwrap().fingerprints.len(),
            1
        );
        let mut updated_catalog = catalog;
        updated_catalog.insert(moved.id, moved.metadata);
        let mut grouping = crate::grouping::Grouping::default();
        grouping.catalog(&updated_catalog);
        let grouped = grouping.group(BTreeMap::from([
            ("path:c:\\app.exe".into(), [1.0]),
            ("app:native:one".into(), [2.0]),
            ("app:native:two".into(), [3.0]),
        ]));
        assert_eq!(grouped.len(), 3);
    }
    #[test]
    fn moved_renamed_file_matches_and_changed_content_does_not() {
        let fixture = crate::test_support::Fixture::new("");
        let first = fixture.root.join("first.exe");
        let moved = fixture.root.join("renamed.exe");
        std::fs::write(&first, b"synthetic executable one").unwrap();
        let initial = identify(&first).unwrap();
        std::fs::rename(&first, &moved).unwrap();
        assert_eq!(identify(&moved).unwrap().0, initial.0);
        std::fs::write(&moved, b"a different executable, not the same application").unwrap();
        assert_ne!(identify(&moved).unwrap().0, initial.0);
        assert!(identify(&first).is_none());
        let host = fixture.root.join("javaw.exe");
        std::fs::write(&host, b"synthetic executable one").unwrap();
        assert!(identify(&host).is_none());
    }
}
