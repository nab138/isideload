// This file was made using https://github.com/Dadoum/Sideloader as a reference.

use crate::Error;
use crate::{
    certificate::CertificateIdentity,
    developer_session::DeveloperDeviceType,
    device::{DeviceInfo, install_app},
};
use std::{io::Write, path::PathBuf};

pub async fn sideload_app(
    handle: &tauri::AppHandle,
    window: &tauri::Window,
    anisette_server: String,
    device: DeviceInfo,
    app_path: PathBuf,
) -> Result<(), Error> {
    if device.uuid.is_empty() {
        return emit_error_and_return(window, "No device selected");
    }
    let dev_session = match crate::sideloader::apple::get_developer_session(
        &handle,
        window,
        anisette_server.clone(),
    )
    .await
    {
        Ok(acc) => acc,
        Err(e) => {
            return emit_error_and_return(
                window,
                &format!("Failed to login to Apple account: {:?}", e),
            );
        }
    };
    let team = match dev_session.get_team().await {
        Ok(t) => t,
        Err(e) => {
            return emit_error_and_return(window, &format!("Failed to get team: {:?}", e));
        }
    };
    window
        .emit("build-output", "Successfully retrieved team".to_string())
        .ok();
    ensure_device_registered(&dev_session, window, &team, &device).await?;

    let config_dir = handle.path().app_config_dir().map_err(|e| e.to_string())?;
    let cert = match CertificateIdentity::new(config_dir, &dev_session, get_apple_email()).await {
        Ok(c) => c,
        Err(e) => {
            return emit_error_and_return(window, &format!("Failed to get certificate: {:?}", e));
        }
    };
    window
        .emit(
            "build-output",
            "Certificate acquired succesfully".to_string(),
        )
        .ok();
    let mut list_app_id_response = match dev_session
        .list_app_ids(DeveloperDeviceType::Ios, &team)
        .await
    {
        Ok(ids) => ids,
        Err(e) => {
            return emit_error_and_return(window, &format!("Failed to list app IDs: {:?}", e));
        }
    };

    let mut app = crate::sideloader::application::Application::new(app_path);
    let is_sidestore = app.bundle.bundle_identifier().unwrap_or("") == "com.SideStore.SideStore";
    let main_app_bundle_id = match app.bundle.bundle_identifier() {
        Some(id) => id.to_string(),
        None => {
            return emit_error_and_return(window, "No bundle identifier found in IPA");
        }
    };
    let main_app_id_str = format!("{}.{}", main_app_bundle_id, team.team_id);
    let main_app_name = match app.bundle.bundle_name() {
        Some(name) => name.to_string(),
        None => {
            return emit_error_and_return(window, "No bundle name found in IPA");
        }
    };

    let extensions = app.bundle.app_extensions_mut();
    // for each extension, ensure it has a unique bundle identifier that starts with the main app's bundle identifier
    for ext in extensions.iter_mut() {
        if let Some(id) = ext.bundle_identifier() {
            if !(id.starts_with(&main_app_bundle_id) && id.len() > main_app_bundle_id.len()) {
                return emit_error_and_return(
                    window,
                    &format!(
                        "Extension {} is not part of the main app bundle identifier: {}",
                        ext.bundle_name().unwrap_or("Unknown"),
                        id
                    ),
                );
            } else {
                ext.set_bundle_identifier(&format!(
                    "{}{}",
                    main_app_id_str,
                    &id[main_app_bundle_id.len()..]
                ));
            }
        }
    }
    app.bundle.set_bundle_identifier(&main_app_id_str);

    let extension_refs: Vec<_> = app.bundle.app_extensions().into_iter().collect();
    let mut bundles_with_app_id = vec![&app.bundle];
    bundles_with_app_id.extend(extension_refs);

    let app_ids_to_register = bundles_with_app_id
        .iter()
        .filter(|bundle| {
            let bundle_id = bundle.bundle_identifier().unwrap_or("");
            !list_app_id_response
                .app_ids
                .iter()
                .any(|app_id| app_id.identifier == bundle_id)
        })
        .collect::<Vec<_>>();

    if app_ids_to_register.len() > list_app_id_response.available_quantity.try_into().unwrap() {
        return emit_error_and_return(
            window,
            &format!(
                "This app requires {} app ids, but you only have {} available",
                app_ids_to_register.len(),
                list_app_id_response.available_quantity
            ),
        );
    }

    for bundle in app_ids_to_register {
        let id = bundle.bundle_identifier().unwrap_or("");
        let name = bundle.bundle_name().unwrap_or("");
        if let Err(e) = dev_session
            .add_app_id(DeveloperDeviceType::Ios, &team, &name, &id)
            .await
        {
            return emit_error_and_return(window, &format!("Failed to register app ID: {:?}", e));
        }
    }
    list_app_id_response = match dev_session
        .list_app_ids(DeveloperDeviceType::Ios, &team)
        .await
    {
        Ok(ids) => ids,
        Err(e) => {
            return emit_error_and_return(window, &format!("Failed to list app IDs: {:?}", e));
        }
    };

    let mut app_ids: Vec<_> = list_app_id_response
        .app_ids
        .into_iter()
        .filter(|app_id| {
            bundles_with_app_id
                .iter()
                .any(|bundle| app_id.identifier == bundle.bundle_identifier().unwrap_or(""))
        })
        .collect();
    let main_app_id = match app_ids
        .iter()
        .find(|app_id| app_id.identifier == main_app_id_str)
        .cloned()
    {
        Some(id) => id,
        None => {
            return emit_error_and_return(
                window,
                &format!(
                    "Main app ID {} not found in registered app IDs",
                    main_app_id_str
                ),
            );
        }
    };

    window
        .emit("build-output", "Registered app IDs".to_string())
        .ok();

    for app_id in app_ids.iter_mut() {
        let app_group_feature_enabled = app_id
            .features
            .get(
                "APG3427HIY", /* Gotta love apple and their magic strings! */
            )
            .and_then(|v| v.as_boolean())
            .ok_or("App group feature not found in app id")?;
        if !app_group_feature_enabled {
            let mut body = plist::Dictionary::new();
            body.insert("APG3427HIY".to_string(), plist::Value::Boolean(true));
            let new_features = match dev_session
                .update_app_id(DeveloperDeviceType::Ios, &team, &app_id, &body)
                .await
            {
                Ok(new_feats) => new_feats,
                Err(e) => {
                    return emit_error_and_return(
                        window,
                        &format!("Failed to update app ID features: {:?}", e),
                    );
                }
            };
            app_id.features = new_features;
        }
    }

    let group_identifier = format!("group.{}", main_app_id_str);

    if is_sidestore {
        app.bundle.app_info.insert(
            "ALTAppGroups".to_string(),
            plist::Value::Array(vec![plist::Value::String(group_identifier.clone())]),
        );
    }

    let app_groups = match dev_session
        .list_application_groups(DeveloperDeviceType::Ios, &team)
        .await
    {
        Ok(groups) => groups,
        Err(e) => {
            return emit_error_and_return(window, &format!("Failed to list app groups: {:?}", e));
        }
    };

    let matching_app_groups = app_groups
        .iter()
        .filter(|group| group.identifier == group_identifier.clone())
        .collect::<Vec<_>>();

    let app_group = if matching_app_groups.is_empty() {
        match dev_session
            .add_application_group(
                DeveloperDeviceType::Ios,
                &team,
                &group_identifier,
                &main_app_name,
            )
            .await
        {
            Ok(group) => group,
            Err(e) => {
                return emit_error_and_return(
                    window,
                    &format!("Failed to register app group: {:?}", e),
                );
            }
        }
    } else {
        matching_app_groups[0].clone()
    };

    //let mut provisioning_profiles: HashMap<String, ProvisioningProfile> = HashMap::new();
    for app_id in app_ids {
        let assign_res = dev_session
            .assign_application_group_to_app_id(
                DeveloperDeviceType::Ios,
                &team,
                &app_id,
                &app_group,
            )
            .await;
        if assign_res.is_err() {
            return emit_error_and_return(
                window,
                &format!(
                    "Failed to assign app group to app ID: {:?}",
                    assign_res.err()
                ),
            );
        }
        // let provisioning_profile = match account
        //     // This doesn't seem right to me, but it's what Sideloader does... Shouldn't it be downloading the provisioning profile for this app ID, not the main?
        //     .download_team_provisioning_profile(DeveloperDeviceType::Ios, &team, &main_app_id)
        //     .await
        // {
        //     Ok(pp /* tee hee */) => pp,
        //     Err(e) => {
        //         return emit_error_and_return(
        //             &window,
        //             &format!("Failed to download provisioning profile: {:?}", e),
        //         );
        //     }
        // };
        // provisioning_profiles.insert(app_id.identifier.clone(), provisioning_profile);
    }

    window
        .emit("build-output", "Registered app groups".to_string())
        .ok();

    let provisioning_profile = match dev_session
        .download_team_provisioning_profile(DeveloperDeviceType::Ios, &team, &main_app_id)
        .await
    {
        Ok(pp /* tee hee */) => pp,
        Err(e) => {
            return emit_error_and_return(
                window,
                &format!("Failed to download provisioning profile: {:?}", e),
            );
        }
    };

    let profile_path = handle
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join(format!("{}.mobileprovision", main_app_id_str));

    if profile_path.exists() {
        std::fs::remove_file(&profile_path).map_err(|e| e.to_string())?;
    }

    let mut file = std::fs::File::create(&profile_path).map_err(|e| e.to_string())?;
    file.write_all(&provisioning_profile.encoded_profile)
        .map_err(|e| e.to_string())?;

    // Without this, zsign complains it can't find the provision file
    #[cfg(target_os = "windows")]
    {
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
    }

    // TODO: Recursive for sub-bundles?
    app.bundle.write_info().map_err(|e| e.to_string())?;

    window
        .emit("build-output", "Signining app...".to_string())
        .ok();

    let zsign_command = handle.shell().sidecar("zsign").unwrap().args([
        "-k",
        cert.get_private_key_file_path().to_str().unwrap(),
        "-c",
        cert.get_certificate_file_path().to_str().unwrap(),
        "-m",
        profile_path.to_str().unwrap(),
        app.bundle.bundle_dir.to_str().unwrap(),
    ]);
    let (mut rx, mut _child) = zsign_command.spawn().expect("Failed to spawn zsign");

    let mut signing_failed = false;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line_bytes) | CommandEvent::Stderr(line_bytes) => {
                let line = String::from_utf8_lossy(&line_bytes);
                window
                    .emit("build-output", Some(line))
                    .expect("failed to emit event");
            }
            CommandEvent::Terminated(result) => {
                if result.code != Some(0) {
                    window
                        .emit("build-output", "App signing failed!".to_string())
                        .ok();
                    signing_failed = true;
                    break;
                }
                window.emit("build-output", "App signed!").ok();

                window
                    .emit(
                        "build-output",
                        "Installing app (Transfer)... 0%".to_string(),
                    )
                    .ok();

                let res = install_app(&device, &app.bundle.bundle_dir, |percentage| {
                    window
                        .emit("build-output", format!("Installing app... {}%", percentage))
                        .expect("failed to emit event");
                })
                .await;
                if let Err(e) = res {
                    window
                        .emit("build-output", format!("Failed to install app: {:?}", e))
                        .ok();
                    signing_failed = true;
                }
                break;
            }
            _ => {}
        }
    }

    if signing_failed {
        return Err("Signing or installation failed".to_string());
    }

    Ok(())
}
