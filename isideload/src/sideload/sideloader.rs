use crate::{
    dev::{
        app_groups::AppGroupsApi,
        app_ids::AppIdsApi,
        developer_session::DeveloperSession,
        devices::DevicesApi,
        teams::{DeveloperTeam, TeamsApi},
    },
    sideload::{
        TeamSelection,
        application::{Application, SpecialApp},
        builder::MaxCertsBehavior,
        cert_identity::CertificateIdentity,
    },
    util::{device::IdeviceInfo, plist::PlistDataExtract, storage::SideloadingStorage},
};

use std::path::PathBuf;

use apple_codesign::{SigningSettings, UnifiedSigner};
use idevice::provider::IdeviceProvider;
use plist::Dictionary;
use plist_macro::plist_to_xml_string;
use rootcause::{option_ext::OptionExt, prelude::*};
use tracing::info;

pub struct Sideloader {
    team_selection: TeamSelection,
    storage: Box<dyn SideloadingStorage>,
    dev_session: DeveloperSession,
    machine_name: String,
    apple_email: String,
    max_certs_behavior: MaxCertsBehavior,
    //extensions_behavior: ExtensionsBehavior,
}

impl Sideloader {
    /// Construct a new `Sideloader` instance with the provided configuration
    ///
    /// See [`crate::sideload::SideloaderBuilder`] for more details and a more convenient way to construct a `Sideloader`.
    pub fn new(
        dev_session: DeveloperSession,
        apple_email: String,
        team_selection: TeamSelection,
        max_certs_behavior: MaxCertsBehavior,
        machine_name: String,
        storage: Box<dyn SideloadingStorage>,
        //extensions_behavior: ExtensionsBehavior,
    ) -> Self {
        Sideloader {
            team_selection,
            storage,
            dev_session,
            machine_name,
            apple_email,
            max_certs_behavior,
            //extensions_behavior,
        }
    }

    /// Sign and install an app
    pub async fn sign_app(
        &mut self,
        app_path: PathBuf,
        team: Option<DeveloperTeam>,
        // this will be replaced with proper entitlement handling later
        increased_memory_limit: bool,
    ) -> Result<PathBuf, Report> {
        let team = match team {
            Some(t) => t,
            None => self.get_team().await?,
        };
        let cert_identity = CertificateIdentity::retrieve(
            &self.machine_name,
            &self.apple_email,
            &mut self.dev_session,
            &team,
            self.storage.as_ref(),
            &self.max_certs_behavior,
        )
        .await
        .context("Failed to retrieve certificate identity")?;

        let mut app = Application::new(app_path)?;
        let special = app.get_special_app();

        let main_bundle_id = app.main_bundle_id()?;
        let main_app_name = app.main_app_name()?;
        let main_app_id_str = format!("{}.{}", main_bundle_id, team.team_id);
        app.update_bundle_id(&main_bundle_id, &main_app_id_str)?;
        let mut app_ids = app
            .register_app_ids(
                /*&self.extensions_behavior, */ &mut self.dev_session,
                &team,
            )
            .await?;
        let main_app_id = match app_ids
            .iter()
            .find(|app_id| app_id.identifier == main_app_id_str)
        {
            Some(id) => id,
            None => {
                bail!(
                    "Main app ID {} not found in registered app IDs",
                    main_app_id_str
                );
            }
        }
        .clone();

        let group_identifier = format!(
            "group.{}",
            if Some(SpecialApp::SideStoreLc) == special {
                format!("com.SideStore.SideStore.{}", team.team_id)
            } else {
                main_app_id_str.clone()
            }
        );

        let app_group = self
            .dev_session
            .ensure_app_group(&team, &main_app_name, &group_identifier, None)
            .await?;

        for app_id in app_ids.iter_mut() {
            app_id
                .ensure_group_feature(&mut self.dev_session, &team)
                .await?;

            self.dev_session
                .assign_app_group(&team, &app_group, app_id, None)
                .await?;

            if increased_memory_limit {
                self.dev_session
                    .add_increased_memory_limit(&team, app_id)
                    .await?;
            }
        }

        app.apply_special_app_behavior(&special, &group_identifier, &cert_identity)
            .await
            .context("Failed to modify app bundle")?;

        let provisioning_profile = self
            .dev_session
            .download_team_provisioning_profile(&team, &main_app_id, None)
            .await?;

        app.bundle.write_info()?;
        for ext in app.bundle.app_extensions_mut() {
            ext.write_info()?;
        }
        for ext in app.bundle.frameworks_mut() {
            ext.write_info()?;
        }

        tokio::fs::write(
            app.bundle.bundle_dir.join("embedded.mobileprovision"),
            provisioning_profile.encoded_profile.as_ref(),
        )
        .await?;

        let mut settings = Self::signing_settings(&cert_identity)?;
        let entitlements: Dictionary = Self::entitlements_from_prov(
            provisioning_profile.encoded_profile.as_ref(),
            &special,
            &team,
        )?;

        settings
            .set_entitlements_xml(
                apple_codesign::SettingsScope::Main,
                plist_to_xml_string(&entitlements),
            )
            .context("Failed to set entitlements XML")?;
        let signer = UnifiedSigner::new(settings);

        for bundle in app.bundle.collect_bundles_sorted() {
            info!("Signing bundle {}", bundle.bundle_dir.display());
            signer
                .sign_path_in_place(&bundle.bundle_dir)
                .context(format!(
                    "Failed to sign bundle: {}",
                    bundle.bundle_dir.display()
                ))?;
        }

        info!("App signed!");

        Ok(app.bundle.bundle_dir.clone())
    }

    #[cfg(feature = "install")]
    pub async fn install_app(
        &mut self,
        device_provider: &impl IdeviceProvider,
        app_path: PathBuf,
        increased_memory_limit: bool,
    ) -> Result<(), Report> {
        let device_info = IdeviceInfo::from_device(device_provider).await?;

        let team = self.get_team().await?;
        self.dev_session
            .ensure_device_registered(&team, &device_info.name, &device_info.udid, None)
            .await?;

        let signed_app_path = self
            .sign_app(app_path, Some(team), increased_memory_limit)
            .await?;

        info!("Installing...");

        crate::sideload::install::install_app(device_provider, &signed_app_path, |progress| {
            info!("Installing: {}%", progress);
        })
        .await
        .context("Failed to install app on device")?;

        Ok(())
    }
    /// Get the developer team according to the configured team selection behavior
    pub async fn get_team(&mut self) -> Result<DeveloperTeam, Report> {
        let teams = self.dev_session.list_teams().await?;
        Ok(match teams.len() {
            0 => {
                bail!("No developer teams available")
            }
            1 => teams.into_iter().next().unwrap(),
            _ => {
                info!(
                    "Multiple developer teams found, {} as per configuration",
                    self.team_selection
                );
                match &self.team_selection {
                    TeamSelection::First => teams.into_iter().next().unwrap(),
                    TeamSelection::Prompt(prompt_fn) => {
                        let selection =
                            prompt_fn(&teams).ok_or_else(|| report!("No team selected"))?;
                        teams
                            .into_iter()
                            .find(|t| t.team_id == selection)
                            .ok_or_else(|| report!("No team found with ID {}", selection))?
                    }
                }
            }
        })
    }

    pub fn signing_settings<'a>(
        cert: &'a CertificateIdentity,
    ) -> Result<SigningSettings<'a>, Report> {
        let mut settings = SigningSettings::default();

        cert.setup_signing_settings(&mut settings)?;
        settings.set_for_notarization(false);
        settings.set_shallow(true);

        Ok(settings)
    }

    fn entitlements_from_prov(
        data: &[u8],
        special: &Option<SpecialApp>,
        team: &DeveloperTeam,
    ) -> Result<Dictionary, Report> {
        let start = data
            .windows(6)
            .position(|w| w == b"<plist")
            .ok_or_report()?;
        let end = data
            .windows(8)
            .rposition(|w| w == b"</plist>")
            .ok_or_report()?
            + 8;
        let plist_data = &data[start..end];
        let plist = plist::Value::from_reader_xml(plist_data)?;

        let mut entitlements = plist
            .as_dictionary()
            .ok_or_report()?
            .get_dict("Entitlements")?
            .clone();

        if matches!(
            special,
            Some(SpecialApp::SideStoreLc) | Some(SpecialApp::LiveContainer)
        ) {
            let mut keychain_access = vec![plist::Value::String(format!(
                "{}.com.kdt.livecontainer.shared",
                team.team_id
            ))];

            for number in 1..128 {
                keychain_access.push(plist::Value::String(format!(
                    "{}.com.kdt.livecontainer.shared.{}",
                    team.team_id, number
                )));
            }

            entitlements.insert(
                "keychain-access-groups".to_string(),
                plist::Value::Array(keychain_access),
            );
        }

        Ok(entitlements)
    }
}
