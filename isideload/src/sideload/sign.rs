use apple_codesign::{
    BundleSigningSettings, ProvisioningProfile, RustCryptoCmsSigner, sign_bundle,
};

use rootcause::prelude::*;

use crate::{
    dev::{app_ids::Profile, teams::DeveloperTeam},
    sideload::{
        application::{Application, SpecialApp},
        cert_identity::CertificateIdentity,
    },
};

pub async fn sign<F, Fut>(
    app: &mut Application,
    cert_identity: &CertificateIdentity,
    provisioning_profile: &Profile,
    special: &Option<SpecialApp>,
    team: &DeveloperTeam,
    progress_callback: Option<F>,
) -> Result<(), Report>
where
    F: Fn(f32) -> Fut,
    Fut: Future<Output = ()>,
{
    let profile = ProvisioningProfile::parse(provisioning_profile.encoded_profile.as_ref())?;
    let certificate_chain = cert_identity.profile_to_certificate_chain(&profile)?;

    let signer = RustCryptoCmsSigner::new(
        cert_identity.private_key.clone(),
        cert_identity.certificate.clone(),
        certificate_chain,
    );

    let mut settings =
        BundleSigningSettings::new(&team.team_id, profile.entitlements().clone(), Some(&signer));
    settings.embedded_mobileprovision = Some(provisioning_profile.encoded_profile.as_ref());
    // let nested_profiles = args
    //     .profiles_by_bundle_id
    //     .iter()
    //     .map(|(bundle_id, path)| {
    //         let data = read_file(path)?;
    //         let profile = ProvisioningProfile::parse(&data)?;
    //         let entitlements = profile.entitlements().clone();
    //         Ok((bundle_id.clone(), data, entitlements))
    //     })
    //     .collect::<Result<Vec<_>>>()?;
    // settings.embedded_mobileprovisions_by_bundle_id = nested_profiles
    //     .iter()
    //     .map(|(bundle_id, data, _)| (bundle_id.clone(), data.as_slice()))
    //     .collect::<BTreeMap<_, _>>();
    // settings.entitlements_by_bundle_id = nested_profiles
    //     .iter()
    //     .map(|(bundle_id, _, entitlements)| (bundle_id.clone(), entitlements.clone()))
    //     .collect();

    Ok(sign_bundle(&app.bundle.bundle_dir, &settings)?)
}
