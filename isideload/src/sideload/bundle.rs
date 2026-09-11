// This file was made using https://github.com/Dadoum/Sideloader as a reference.
// I'm planning on redoing this later to better handle entitlements, extensions, etc, but it will do for now

use isideload_vfs::fs::File;
use plist::{Dictionary, Value, to_writer_binary};
use rootcause::prelude::*;
use std::{
    io::BufWriter,
    path::{Path, PathBuf},
};

use crate::SideloadError;

#[derive(Debug, Clone)]
pub struct Bundle {
    pub app_info: Dictionary,
    pub bundle_dir: PathBuf,

    app_extensions: Vec<Bundle>,
    frameworks: Vec<Bundle>,
    watch_apps: Vec<Bundle>,
    _libraries: Vec<String>,
}

impl Bundle {
    pub fn new(bundle_dir: PathBuf) -> Result<Self, Report> {
        let mut bundle_path = bundle_dir;
        // Remove trailing slash/backslash
        if let Some(path_str) = bundle_path.to_str()
            && (path_str.ends_with('/') || path_str.ends_with('\\'))
        {
            bundle_path = PathBuf::from(&path_str[..path_str.len() - 1]);
        }

        let info_plist_path = bundle_path.join("Info.plist");
        assert_bundle(
            isideload_vfs::fs::metadata(&info_plist_path).is_ok(),
            &format!("No Info.plist here: {}", info_plist_path.display()),
        )?;

        let plist_data = isideload_vfs::fs::read(&info_plist_path).context(
            SideloadError::InvalidBundle("Failed to read Info.plist".to_string()),
        )?;

        let app_info = plist::from_bytes(&plist_data).context(SideloadError::InvalidBundle(
            "Failed to parse Info.plist".to_string(),
        ))?;

        let plug_ins_dir = bundle_path.join("PlugIns");
        let app_extensions = if isideload_vfs::fs::metadata(&plug_ins_dir).is_ok() {
            isideload_vfs::fs::read_dir(&plug_ins_dir)
                .context(SideloadError::InvalidBundle(
                    "Failed to read PlugIns directory".to_string(),
                ))?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                        && isideload_vfs::fs::metadata(&entry.path().join("Info.plist")).is_ok()
                })
                .filter_map(|entry| Bundle::new(entry.path()).ok())
                .collect()
        } else {
            Vec::new()
        };

        let frameworks_dir = bundle_path.join("Frameworks");
        let frameworks = if isideload_vfs::fs::metadata(&frameworks_dir).is_ok() {
            isideload_vfs::fs::read_dir(&frameworks_dir)
                .context(SideloadError::InvalidBundle(
                    "Failed to read Frameworks directory".to_string(),
                ))?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                        && isideload_vfs::fs::metadata(&entry.path().join("Info.plist")).is_ok()
                })
                .filter_map(|entry| Bundle::new(entry.path()).ok())
                .collect()
        } else {
            Vec::new()
        };

        let watch_dir = bundle_path.join("Watch");
        let watch_apps = if isideload_vfs::fs::metadata(&watch_dir).is_ok() {
            isideload_vfs::fs::read_dir(&watch_dir)
                .context(SideloadError::InvalidBundle(
                    "Failed to read Watch directory".to_string(),
                ))?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                        && entry.path().extension().is_some_and(|ext| ext == "app")
                        && isideload_vfs::fs::metadata(&entry.path().join("Info.plist")).is_ok()
                })
                .filter_map(|entry| Bundle::new(entry.path()).ok())
                .collect()
        } else {
            Vec::new()
        };

        let libraries = find_dylibs(&bundle_path, &bundle_path)?;

        Ok(Bundle {
            app_info,
            bundle_dir: bundle_path,
            app_extensions,
            frameworks,
            watch_apps,
            _libraries: libraries,
        })
    }

    pub fn set_bundle_identifier(&mut self, id: &str) {
        self.app_info.insert(
            "CFBundleIdentifier".to_string(),
            Value::String(id.to_string()),
        );
    }

    pub fn bundle_identifier(&self) -> Option<&str> {
        self.app_info
            .get("CFBundleIdentifier")
            .and_then(|v| v.as_string())
    }

    pub fn bundle_name(&self) -> Option<&str> {
        self.app_info
            .get("CFBundleName")
            .and_then(|v| v.as_string())
    }

    pub fn app_extensions(&self) -> &[Bundle] {
        &self.app_extensions
    }

    pub fn app_extensions_mut(&mut self) -> &mut [Bundle] {
        &mut self.app_extensions
    }

    pub fn frameworks(&self) -> &[Bundle] {
        &self.frameworks
    }

    pub fn frameworks_mut(&mut self) -> &mut [Bundle] {
        &mut self.frameworks
    }

    pub fn watch_apps(&self) -> &[Bundle] {
        &self.watch_apps
    }

    pub fn watch_apps_mut(&mut self) -> &mut [Bundle] {
        &mut self.watch_apps
    }

    fn rewrite_child_bundle_id(
        child: &mut Bundle,
        main_app_bundle_id: &str,
        main_app_id_str: &str,
        kind: &str,
    ) -> Result<(), Report> {
        let id = child
            .bundle_identifier()
            .ok_or_else(|| report!("{} bundle is missing CFBundleIdentifier", kind))?
            .to_string();

        if !(id.starts_with(main_app_bundle_id) && id.len() > main_app_bundle_id.len()) {
            bail!(SideloadError::InvalidBundle(format!(
                "{} {} is not part of the main app bundle identifier: {}",
                kind,
                child.bundle_name().unwrap_or("Unknown"),
                id
            )));
        }

        child.set_bundle_identifier(&format!(
            "{}{}",
            main_app_id_str,
            &id[main_app_bundle_id.len()..]
        ));
        child.rewrite_embedded_bundle_ids(main_app_bundle_id, main_app_id_str)?;

        Ok(())
    }

    pub fn rewrite_embedded_bundle_ids(
        &mut self,
        main_app_bundle_id: &str,
        main_app_id_str: &str,
    ) -> Result<(), Report> {
        for ext in self.app_extensions.iter_mut() {
            Self::rewrite_child_bundle_id(ext, main_app_bundle_id, main_app_id_str, "Extension")?;
        }

        for watch_app in self.watch_apps.iter_mut() {
            if let Some(companion_id) = watch_app
                .app_info
                .get("WKCompanionAppBundleIdentifier")
                .and_then(|value| value.as_string())
                && companion_id != main_app_bundle_id
            {
                bail!(SideloadError::InvalidBundle(format!(
                    "Watch app {} references companion bundle identifier {}, expected {}",
                    watch_app.bundle_name().unwrap_or("Unknown"),
                    companion_id,
                    main_app_bundle_id
                )));
            }

            watch_app.app_info.insert(
                "WKCompanionAppBundleIdentifier".to_string(),
                Value::String(main_app_id_str.to_string()),
            );

            Self::rewrite_child_bundle_id(
                watch_app,
                main_app_bundle_id,
                main_app_id_str,
                "Watch app",
            )?;
        }

        Ok(())
    }

    fn collect_app_id_bundles_into<'a>(&'a self, bundles: &mut Vec<&'a Bundle>) {
        for bundle in &self.app_extensions {
            bundles.push(bundle);
            bundle.collect_app_id_bundles_into(bundles);
        }
        for bundle in &self.watch_apps {
            bundles.push(bundle);
            bundle.collect_app_id_bundles_into(bundles);
        }
    }

    pub fn collect_app_id_bundles(&self) -> Vec<&Bundle> {
        let mut bundles = vec![self];
        self.collect_app_id_bundles_into(&mut bundles);
        bundles
    }

    pub fn write_info(&self) -> Result<(), Report> {
        let info_plist_path = self.bundle_dir.join("Info.plist");
        let mut file = File::create(&info_plist_path).context(SideloadError::InvalidBundle(
            "Failed to write Info.plist".to_string(),
        ))?;
        to_writer_binary(BufWriter::new(&mut file), &self.app_info).context(
            SideloadError::InvalidBundle("Failed to create Info.plist writer".to_string()),
        )?;

        file.sync_all().context(SideloadError::InvalidBundle(
            "Failed to sync Info.plist".to_string(),
        ))?;
        Ok(())
    }

    pub fn write_info_recursive(&self) -> Result<(), Report> {
        self.write_info()?;
        for bundle in &self.app_extensions {
            bundle.write_info_recursive()?;
        }
        for bundle in &self.frameworks {
            bundle.write_info_recursive()?;
        }
        for bundle in &self.watch_apps {
            bundle.write_info_recursive()?;
        }
        Ok(())
    }

    fn from_dylib_path(dylib_path: PathBuf) -> Self {
        Self {
            app_info: Dictionary::new(),
            bundle_dir: dylib_path,
            app_extensions: Vec::new(),
            frameworks: Vec::new(),
            watch_apps: Vec::new(),
            _libraries: Vec::new(),
        }
    }

    fn collect_dylib_bundles(&self) -> Vec<Bundle> {
        self._libraries
            .iter()
            .map(|relative| Self::from_dylib_path(self.bundle_dir.join(relative)))
            .collect()
    }

    fn collect_nested_bundles_into(&self, bundles: &mut Vec<Bundle>) {
        for bundle in &self.app_extensions {
            bundles.push(bundle.clone());
            bundle.collect_nested_bundles_into(bundles);
        }
        for bundle in &self.frameworks {
            bundles.push(bundle.clone());
            bundle.collect_nested_bundles_into(bundles);
        }
        for bundle in &self.watch_apps {
            bundles.push(bundle.clone());
            bundle.collect_nested_bundles_into(bundles);
        }
    }

    pub fn collect_nested_bundles(&self) -> Vec<Bundle> {
        let mut bundles = Vec::new();
        self.collect_nested_bundles_into(&mut bundles);
        bundles.extend(self.collect_dylib_bundles());
        bundles
    }

    pub fn collect_bundles_sorted(&self) -> Vec<Bundle> {
        let mut bundles = self.collect_nested_bundles();
        bundles.push(self.clone());
        bundles.sort_by_key(|b| b.bundle_dir.components().count());
        bundles.reverse();
        bundles
    }
}

fn assert_bundle(condition: bool, msg: &str) -> Result<(), Report> {
    if !condition {
        bail!(SideloadError::InvalidBundle(msg.to_string()))
    } else {
        Ok(())
    }
}

fn find_dylibs(dir: &Path, bundle_root: &Path) -> Result<Vec<String>, Report> {
    let mut libraries = Vec::new();

    fn collect_dylibs(
        dir: &Path,
        bundle_root: &Path,
        libraries: &mut Vec<String>,
    ) -> Result<(), Report> {
        let entries = isideload_vfs::fs::read_dir(dir).context(SideloadError::InvalidBundle(
            format!("Failed to read directory {}", dir.display()),
        ))?;

        for entry in entries {
            let entry = entry.context(SideloadError::InvalidBundle(
                "Failed to read directory entry".to_string(),
            ))?;

            let path = entry.path();
            let file_type = isideload_vfs::fs::metadata(&path)
                .map(|m| m.file_type())
                .context(SideloadError::InvalidBundle(
                    "Failed to get file type".to_string(),
                ))?;

            if file_type.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && name.ends_with(".dylib")
                {
                    if let Ok(relative_path) = path.strip_prefix(bundle_root)
                        && let Some(relative_str) = relative_path.to_str()
                    {
                        libraries.push(relative_str.to_string());
                    }
                }
            } else if file_type.is_dir() {
                collect_dylibs(&path, bundle_root, libraries)?;
            }
        }
        Ok(())
    }

    collect_dylibs(dir, bundle_root, &mut libraries)?;
    Ok(libraries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bundle(id: &str, name: &str) -> Bundle {
        let mut app_info = Dictionary::new();
        app_info.insert(
            "CFBundleIdentifier".to_string(),
            Value::String(id.to_string()),
        );
        app_info.insert(
            "CFBundleName".to_string(),
            Value::String(name.to_string()),
        );

        Bundle {
            app_info,
            bundle_dir: PathBuf::from(name),
            app_extensions: Vec::new(),
            frameworks: Vec::new(),
            watch_apps: Vec::new(),
            _libraries: Vec::new(),
        }
    }

    #[test]
    fn rewrites_watch_bundle_and_companion_identifier() {
        let mut root = test_bundle("com.example.app", "Main");
        let mut watch = test_bundle("com.example.app.watchkitapp", "Watch");
        watch.app_info.insert(
            "WKCompanionAppBundleIdentifier".to_string(),
            Value::String("com.example.app".to_string()),
        );
        root.watch_apps.push(watch);

        root.rewrite_embedded_bundle_ids("com.example.app", "com.example.app.TEAM")
            .unwrap();
        root.set_bundle_identifier("com.example.app.TEAM");

        assert_eq!(root.bundle_identifier(), Some("com.example.app.TEAM"));
        assert_eq!(
            root.watch_apps[0].bundle_identifier(),
            Some("com.example.app.TEAM.watchkitapp")
        );
        assert_eq!(
            root.watch_apps[0]
                .app_info
                .get("WKCompanionAppBundleIdentifier")
                .and_then(|value| value.as_string()),
            Some("com.example.app.TEAM")
        );

        let app_ids = root.collect_app_id_bundles();
        assert_eq!(app_ids.len(), 2);
        assert_eq!(
            app_ids[1].bundle_identifier(),
            Some("com.example.app.TEAM.watchkitapp")
        );
    }
}
