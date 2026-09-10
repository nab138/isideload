// This file was made using https://github.com/Dadoum/Sideloader as a reference.
// I'm planning on redoing this later to better handle entitlements, extensions, etc, but it will do for now

use crate::SideloadError;
use crate::dev::app_ids::{AppId, AppIdsApi};
use crate::dev::developer_session::DeveloperSession;
use crate::dev::teams::DeveloperTeam;
use crate::sideload::bundle::Bundle;
use crate::sideload::cert_identity::CertificateIdentity;
use isideload_vfs::fs::File;
use rootcause::option_ext::OptionExt;
use rootcause::prelude::*;
use std::io::Write;
use std::path::PathBuf;
use tracing::{info, warn};
use zip::ZipArchive;

pub struct Application {
    pub bundle: Bundle,
    //pub temp_path: PathBuf,
}

impl Application {
    pub fn new(path: PathBuf) -> Result<Self, Report> {
        if !isideload_vfs::fs::metadata(&path).is_ok() {
            bail!(SideloadError::InvalidBundle(
                "Application path does not exist".to_string(),
            ));
        }

        let mut bundle_path = path.clone();
        //let mut temp_path = PathBuf::new();

        if isideload_vfs::fs::metadata(&bundle_path)?.is_file() {
            let temp_dir = isideload_vfs::fs::temp_dir();
            let temp_path = temp_dir.join(
                path.file_name()
                    .ok_or_report()?
                    .to_string_lossy()
                    .to_string()
                    + "_extracted",
            );
            if isideload_vfs::fs::metadata(&temp_path).is_ok() {
                isideload_vfs::fs::remove_dir_all(&temp_path)
                    .context("Failed to remove existing temporary directory")?;
            }
            isideload_vfs::fs::create_dir_all(&temp_path)
                .context("Failed to create temporary directory")?;

            let file = File::open(&path).context("Failed to open application archive")?;
            let mut archive =
                ZipArchive::new(file).context("Failed to open application archive")?;

            archive
                .extract(&temp_path)
                .context("Failed to extract application archive")?;

            let payload_folder = temp_path.join("Payload");
            if isideload_vfs::fs::metadata(&payload_folder).is_ok() && payload_folder.is_dir() {
                let app_dirs: Vec<_> = isideload_vfs::fs::read_dir(&payload_folder)
                    .context("Failed to read Payload directory")?
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "app"))
                    .collect();
                if app_dirs.len() == 1 {
                    bundle_path = app_dirs[0].path();
                } else if app_dirs.is_empty() {
                    bail!(SideloadError::InvalidBundle(
                        "No .app directory found in Payload".to_string(),
                    ));
                } else {
                    bail!(SideloadError::InvalidBundle(
                        "Multiple .app directories found in Payload".to_string(),
                    ));
                }
            } else {
                // gather the directory contents as a string for debugging
                let mut contents = String::new();
                if isideload_vfs::fs::metadata(&temp_path).is_ok() && temp_path.is_dir() {
                    let entries = isideload_vfs::fs::read_dir(&temp_path)
                        .context("Failed to read temporary directory for error reporting")?;
                    for entry in entries {
                        if let Ok(entry) = entry {
                            contents
                                .push_str(&format!("{}\n", entry.file_name().to_string_lossy()));
                        }
                    }
                }
                bail!(SideloadError::InvalidBundle(format!(
                    "No Payload directory found in the application archive, instead: {}",
                    contents
                ),));
            }
        }
        let bundle = Bundle::new(bundle_path)?;

        Ok(Application {
            bundle, /*temp_path*/
        })
    }

    pub fn get_special_app(&self) -> Option<SpecialApp> {
        let bundle_id = self.bundle.bundle_identifier().unwrap_or("");
        let special_app = match bundle_id {
            "com.rileytestut.AltStore" => Some(SpecialApp::AltStore),
            "com.SideStore.SideStore" => Some(SpecialApp::SideStore),
            "app.stik.store" => Some(SpecialApp::StikStore),
            _ => None,
        };
        if special_app.is_some() {
            return special_app;
        }

        if self
            .bundle
            .frameworks()
            .iter()
            .any(|f| f.bundle_identifier().unwrap_or("") == "com.SideStore.SideStore")
        {
            return Some(SpecialApp::SideStoreLc);
        }

        if bundle_id == "com.kdt.livecontainer" {
            return Some(SpecialApp::LiveContainer);
        }

        None
    }

    pub fn main_bundle_id(&self) -> Result<String, Report> {
        let str = self
            .bundle
            .bundle_identifier()
            .ok_or_report()
            .context("Failed to get main bundle identifier")?
            .to_string();

        Ok(str)
    }

    pub fn main_app_name(&self) -> Result<String, Report> {
        let str = self
            .bundle
            .bundle_name()
            .ok_or_report()
            .context("Failed to get main app name")?
            .to_string();

        Ok(str)
    }

    pub fn update_bundle_id(
        &mut self,
        main_app_bundle_id: &str,
        main_app_id_str: &str,
    ) -> Result<(), Report> {
        let extensions = self.bundle.app_extensions_mut();
        for ext in extensions.iter_mut() {
            if let Some(id) = ext.bundle_identifier() {
                if !(id.starts_with(main_app_bundle_id) && id.len() > main_app_bundle_id.len()) {
                    bail!(SideloadError::InvalidBundle(format!(
                        "Extension {} is not part of the main app bundle identifier: {}",
                        ext.bundle_name().unwrap_or("Unknown"),
                        id
                    )));
                } else {
                    ext.set_bundle_identifier(&format!(
                        "{}{}",
                        main_app_id_str,
                        &id[main_app_bundle_id.len()..]
                    ));
                }
            }
        }
        self.bundle.set_bundle_identifier(main_app_id_str);

        Ok(())
    }

    fn unregistered_bundles(&self, app_ids: &[AppId]) -> Vec<&Bundle> {
        std::iter::once(&self.bundle)
            .chain(self.bundle.app_extensions())
            .filter(|bundle| {
                let identifier = bundle.bundle_identifier().unwrap_or("");
                !app_ids
                    .iter()
                    .any(|app_id| app_id.identifier.eq_ignore_ascii_case(identifier))
            })
            .collect()
    }

    fn resolve_app_ids(&self, app_ids: &[AppId]) -> Result<Vec<AppId>, Report> {
        std::iter::once(&self.bundle)
            .chain(self.bundle.app_extensions())
            .map(|bundle| -> Result<AppId, Report> {
                let identifier = bundle.bundle_identifier().unwrap_or("");
                Ok(app_ids
                    .iter()
                    .find(|app_id| app_id.identifier.eq_ignore_ascii_case(identifier))
                    .cloned()
                    .ok_or_report()
                    .context(format!(
                        "Registered app ID not found for bundle {}",
                        identifier
                    ))?)
            })
            .collect()
    }

    pub(crate) fn canonicalize_bundle_ids(&mut self, app_ids: &[AppId]) -> Result<(), Report> {
        // Resolve every bundle before mutating any of them. Apple treats IDs as
        // case-insensitive, but the signer's profile/entitlement maps use exact keys.
        let resolved = self.resolve_app_ids(app_ids)?;
        self.bundle.set_bundle_identifier(&resolved[0].identifier);
        for (extension, app_id) in self
            .bundle
            .app_extensions_mut()
            .iter_mut()
            .zip(resolved.iter().skip(1))
        {
            extension.set_bundle_identifier(&app_id.identifier);
        }
        Ok(())
    }

    pub async fn register_app_ids(
        &self,
        //mode: &ExtensionsBehavior,
        dev_session: &mut DeveloperSession,
        team: &DeveloperTeam,
    ) -> Result<Vec<AppId>, Report> {
        let list_app_ids_response = dev_session
            .list_app_ids(team, None)
            .await
            .context("Failed to list app IDs for the developer team")?;
        let app_ids_to_register = self.unregistered_bundles(&list_app_ids_response.app_ids);

        if let Some(available) = list_app_ids_response.available_quantity {
            if available < 0 {
                warn!(
                    "Apple reports a negative number of available app IDs ({}), which shouldn't be possible.",
                    available
                );
                // Since the App IDs should never be negative in the first place, it might still be worth trying to register them anyways. Who knows.
            } else {
                // We only do the conversion if available is positive, else we get an integral conversion error
                if app_ids_to_register.len() > available.try_into()? {
                    bail!(
                        "Not enough available app IDs. {} {} required, but only {} {} available.",
                        app_ids_to_register.len(),
                        if app_ids_to_register.len() == 1 {
                            "is"
                        } else {
                            "are"
                        },
                        available,
                        if available == 1 { "is" } else { "are" }
                    );
                }
            }
        }

        for bundle in app_ids_to_register {
            let id = bundle.bundle_identifier().unwrap_or("");
            let name = bundle.bundle_name().unwrap_or("");
            dev_session.add_app_id(team, name, id, None).await?;
        }
        let list_app_id_response = dev_session.list_app_ids(team, None).await?;
        let app_ids = self.resolve_app_ids(&list_app_id_response.app_ids)?;

        info!("Registered app IDs");
        Ok(app_ids)
    }

    pub async fn apply_special_app_behavior(
        &mut self,
        special: &Option<SpecialApp>,
        group_identifier: &str,
        cert: &CertificateIdentity,
    ) -> Result<(), Report> {
        let Some(special) = special.as_ref() else {
            return Ok(());
        };

        if matches!(
            special,
            SpecialApp::SideStoreLc
                | SpecialApp::SideStore
                | SpecialApp::AltStore
                | SpecialApp::StikStore
        ) {
            if !matches!(special, SpecialApp::StikStore) {
                self.bundle.app_info.insert(
                    "ALTAppGroups".to_string(),
                    plist::Value::Array(vec![plist::Value::String(group_identifier.to_string())]),
                );
            }
            info!("Injecting certificate for {}", special);

            let target_bundle =
                match special {
                    SpecialApp::SideStoreLc => self.bundle.frameworks_mut().iter_mut().find(|fw| {
                        fw.bundle_identifier().unwrap_or("") == "com.SideStore.SideStore"
                    }),
                    _ => Some(&mut self.bundle),
                };

            if let Some(target_bundle) = target_bundle {
                let id_key = match special {
                    SpecialApp::StikStore => "MachineID",
                    _ => "ALTCertificateID",
                };
                let cert_file_name = match special {
                    SpecialApp::StikStore => "Certificate.p12",
                    _ => "ALTCertificate.p12",
                };
                target_bundle.app_info.insert(
                    id_key.to_string(),
                    plist::Value::String(cert.get_serial_number()),
                );

                let p12_bytes = cert
                    .as_p12(&cert.machine_id)
                    .await
                    .context("Failed to encode cert as p12")?;
                let alt_cert_path = target_bundle.bundle_dir.join(cert_file_name);

                let mut file = isideload_vfs::fs::File::create(&alt_cert_path)
                    .context(format!("Failed to create {}", cert_file_name))?;
                file.write_all(&p12_bytes)
                    .context(format!("Failed to write {}", cert_file_name))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialApp {
    SideStore,
    SideStoreLc,
    LiveContainer,
    AltStore,
    StikStore,
}

// impl display
impl std::fmt::Display for SpecialApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecialApp::SideStore => write!(f, "SideStore"),
            SpecialApp::SideStoreLc => write!(f, "SideStore+LiveContainer"),
            SpecialApp::LiveContainer => write!(f, "LiveContainer"),
            SpecialApp::AltStore => write!(f, "AltStore"),
            SpecialApp::StikStore => write!(f, "StikStore"),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod app_id_tests {
    use super::*;
    use plist::{Dictionary, Value};
    use std::collections::BTreeMap;
    use std::path::Path;

    struct Fixture {
        root: PathBuf,
        app: Application,
    }

    impl Fixture {
        fn new(main_id: &str, extension_ids: &[&str]) -> Self {
            let root = std::env::temp_dir()
                .join(format!("isideload-app-id-test-{}", uuid::Uuid::new_v4()));
            let main = root.join("Video.app");
            write_bundle(&main, main_id);
            for (index, identifier) in extension_ids.iter().enumerate() {
                write_bundle(
                    &main.join("PlugIns").join(format!("Extension{index}.appex")),
                    identifier,
                );
            }
            Self {
                app: Application::new(main).expect("load fixture"),
                root,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn write_bundle(path: &Path, identifier: &str) {
        std::fs::create_dir_all(path).expect("create fixture directory");
        let mut info = Dictionary::new();
        info.insert(
            "CFBundleIdentifier".to_string(),
            Value::String(identifier.to_string()),
        );
        info.insert(
            "CFBundleName".to_string(),
            Value::String("Video".to_string()),
        );
        plist::to_file_xml(path.join("Info.plist"), &info).expect("write fixture plist");
    }

    fn registered(identifier: &str) -> AppId {
        AppId {
            app_id_id: format!("registered-{identifier}"),
            identifier: identifier.to_string(),
            name: "Video".to_string(),
            features: Dictionary::new(),
            expiration_date: None,
        }
    }

    #[test]
    fn extension_case_difference_reuses_registration_and_exact_profile_key() {
        let main = "com.example.video.TEAM123456";
        let requested = "com.example.video.TEAM123456.OpenYouTube.Extension";
        let canonical = "com.example.video.TEAM123456.OpenYoutube.Extension";
        let mut fixture = Fixture::new(main, &[requested]);
        let app_ids = vec![registered(main), registered(canonical)];

        // A casing difference must not consume another App ID slot or call addAppId.
        assert!(fixture.app.unregistered_bundles(&app_ids).is_empty());
        let resolved = fixture.app.resolve_app_ids(&app_ids).expect("resolve IDs");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[1].app_id_id, app_ids[1].app_id_id);

        fixture
            .app
            .canonicalize_bundle_ids(&resolved)
            .expect("canonicalize");
        let extension = &fixture.app.bundle.app_extensions()[0];
        assert_eq!(extension.bundle_identifier(), Some(canonical));

        // sign.rs builds maps using Apple's returned identifiers as exact keys.
        let profile_keys: BTreeMap<_, _> = resolved
            .iter()
            .map(|id| (id.identifier.as_str(), id.app_id_id.as_str()))
            .collect();
        assert!(profile_keys.contains_key(extension.bundle_identifier().unwrap()));

        extension.write_info().expect("write canonical identifier");
        let reloaded = Bundle::new(extension.bundle_dir.clone()).expect("reload plist");
        assert_eq!(reloaded.bundle_identifier(), Some(canonical));
    }

    #[test]
    fn main_and_extension_use_registered_spelling_without_lowercasing() {
        let mut fixture = Fixture::new(
            "com.example.video.TEAM123456",
            &["com.example.video.TEAM123456.ShareExtension"],
        );
        let main = "com.Example.Video.TEAM123456";
        let extension = "com.Example.Video.TEAM123456.shareExtension";
        // Response order need not match bundle order.
        let app_ids = vec![registered(extension), registered(main)];

        assert!(fixture.app.unregistered_bundles(&app_ids).is_empty());
        fixture
            .app
            .canonicalize_bundle_ids(&app_ids)
            .expect("canonicalize");
        assert_eq!(fixture.app.main_bundle_id().unwrap(), main);
        assert_eq!(
            fixture.app.bundle.app_extensions()[0].bundle_identifier(),
            Some(extension)
        );
        assert!(
            app_ids
                .iter()
                .any(|id| id.identifier == fixture.app.main_bundle_id().unwrap())
        );
    }

    #[test]
    fn genuinely_new_identifier_is_still_registered() {
        let main = "com.example.video.TEAM123456";
        let fresh = "com.example.video.TEAM123456.OpenYouTube2.Extension";
        let fixture = Fixture::new(main, &[fresh]);
        let app_ids = vec![
            registered(main),
            registered("com.example.video.TEAM123456.OpenYoutube.Extension"),
        ];
        let missing = fixture.app.unregistered_bundles(&app_ids);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].bundle_identifier(), Some(fresh));
    }

    #[test]
    fn missing_registration_fails_before_mutating_bundles() {
        let main = "com.example.video.TEAM123456";
        let extension = "com.example.video.TEAM123456.ShareExtension";
        let mut fixture = Fixture::new(main, &[extension]);
        let only_main = vec![registered("com.Example.Video.TEAM123456")];

        assert!(fixture.app.canonicalize_bundle_ids(&only_main).is_err());
        assert_eq!(fixture.app.main_bundle_id().unwrap(), main);
        assert_eq!(
            fixture.app.bundle.app_extensions()[0].bundle_identifier(),
            Some(extension)
        );
    }

    #[test]
    fn exact_matches_and_unrelated_registered_ids_are_preserved() {
        let main = "com.example.video.TEAM123456";
        let fixture = Fixture::new(main, &[]);
        let app_ids = vec![registered("com.unrelated.app"), registered(main)];
        let resolved = fixture.app.resolve_app_ids(&app_ids).expect("resolve");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].identifier, main);
        assert!(fixture.app.unregistered_bundles(&app_ids).is_empty());
    }
}
