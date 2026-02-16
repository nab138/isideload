use apple_codesign::{SigningSettings, UnifiedSigner};
use plist::Dictionary;
use plist_macro::plist_to_xml_string;
use rootcause::{option_ext::OptionExt, prelude::*};
use tracing::info;
use x509_certificate::{CapturedX509Certificate, KeyInfoSigner};

use crate::{
    sideload::{
        application::{Application, SpecialApp},
        cert_identity::CertificateIdentity,
    },
    util::plist::PlistDataExtract,
};

pub fn sign(
    mut settings: SigningSettings,
    app: &mut Application,
    provisioning_profile: &[u8],
    special: &Option<SpecialApp>,
    team_id: &str,
) -> Result<(), Report> {
    let entitlements: Dictionary = entitlements_from_prov(provisioning_profile, special, team_id)?;

    settings
        .set_entitlements_xml(
            apple_codesign::SettingsScope::Main,
            plist_to_xml_string(&entitlements),
        )
        .context("Failed to set entitlements XML")?;
    let signer = UnifiedSigner::new(settings);

    for bundle in app.bundle.collect_bundles_sorted() {
        info!(
            "Signing {}",
            bundle
                .bundle_dir
                .file_name()
                .unwrap_or(bundle.bundle_dir.as_os_str())
                .to_string_lossy()
        );
        signer
            .sign_path_in_place(&bundle.bundle_dir)
            .context(format!(
                "Failed to sign bundle: {}",
                bundle.bundle_dir.display()
            ))?;
    }

    Ok(())
}

pub fn signing_settings<'a>(cert: &'a CertificateIdentity) -> Result<SigningSettings<'a>, Report> {
    let mut settings = SigningSettings::default();

    cert.setup_signing_settings(&mut settings)?;
    settings.set_for_notarization(false);
    settings.set_shallow(true);

    Ok(settings)
}

pub fn imported_cert_signing_settings<'a, T: KeyInfoSigner>(
    key: &'a T,
    cert: CapturedX509Certificate,
) -> Result<SigningSettings<'a>, Report> {
    let mut settings = SigningSettings::default();

    settings.set_signing_key(key, cert);

    settings.set_for_notarization(false);
    settings.set_shallow(true);
    settings.chain_apple_certificates();
    settings.set_team_id_from_signing_certificate();
    Ok(settings)
}

fn entitlements_from_prov(
    data: &[u8],
    special: &Option<SpecialApp>,
    team_id: &str,
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
            team_id
        ))];

        for number in 1..128 {
            keychain_access.push(plist::Value::String(format!(
                "{}.com.kdt.livecontainer.shared.{}",
                team_id, number
            )));
        }

        entitlements.insert(
            "keychain-access-groups".to_string(),
            plist::Value::Array(keychain_access),
        );
    }

    Ok(entitlements)
}
