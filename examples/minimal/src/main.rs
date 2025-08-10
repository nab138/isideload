use std::{env, path::PathBuf, sync::Arc};

use isideload::{
    AnisetteConfiguration, AppleAccount, DefaultLogger, DeveloperSession, device::list_devices,
    sideload::sideload_app,
};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let app_path = PathBuf::from(
        args.get(1)
            .expect("Please provide the path to the app to install"),
    );
    let apple_id = args
        .get(2)
        .expect("Please provide the Apple ID to use for installation");
    let apple_password = args.get(3).expect("Please provide the Apple ID password");

    // You don't have to use the builtin list_devices method if you don't want to use usbmuxd
    // You can use idevice to get the device info however you want
    // This is just easier
    let device = list_devices().await.unwrap().into_iter().next().unwrap();
    println!("Target device: {}", device.name);

    // Change the anisette url and such here
    // Note that right now only remote anisette servers are supported
    let anisette_config = AnisetteConfiguration::default();

    let get_2fa_code = || {
        let mut code = String::new();
        println!("Enter 2FA code:");
        std::io::stdin().read_line(&mut code).unwrap();
        Ok(code.trim().to_string())
    };

    let account = AppleAccount::login(
        || Ok((apple_id.to_string(), apple_password.to_string())),
        get_2fa_code,
        anisette_config,
    )
    .await
    .unwrap();

    let dev_session = DeveloperSession::new(Arc::new(account));

    // This is where certificates, mobileprovision, and anisette data will be stored
    let store_dir = std::env::current_dir().unwrap();

    sideload_app(DefaultLogger {}, &dev_session, &device, app_path, store_dir)
        .await
        .unwrap()
}
