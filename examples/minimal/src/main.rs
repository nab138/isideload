use std::{env, path::PathBuf};

use idevice::usbmuxd::{UsbmuxdAddr, UsbmuxdConnection};
use isideload::{
    anisette::remote_v3::RemoteV3AnisetteProvider,
    auth::apple_account::AppleAccount,
    dev::developer_session::DeveloperSession,
    sideload::{SideloadConfiguration, TeamSelection, sideload_app},
};

use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    isideload::init().expect("Failed to initialize error reporting");
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args: Vec<String> = env::args().collect();

    let apple_id = args
        .get(1)
        .expect("Please provide the Apple ID to use for installation");
    let apple_password = args.get(2).expect("Please provide the Apple ID password");
    let app_path = PathBuf::from(
        args.get(3)
            .expect("Please provide the path to the app to install"),
    );

    let get_2fa_code = || {
        let mut code = String::new();
        println!("Enter 2FA code:");
        std::io::stdin().read_line(&mut code).unwrap();
        Some(code.trim().to_string())
    };

    let account = AppleAccount::builder(apple_id)
        .anisette_provider(RemoteV3AnisetteProvider::default().set_serial_number("2".to_string()))
        .login(apple_password, get_2fa_code)
        .await;

    match &account {
        Ok(a) => println!("Logged in. {}", a),
        Err(e) => panic!("Failed to log in to Apple ID: {:?}", e),
    }

    let mut account = account.unwrap();

    let mut dev_session = DeveloperSession::from_account(&mut account)
        .await
        .expect("Failed to create developer session");

    let usbmuxd = UsbmuxdConnection::default().await;
    if usbmuxd.is_err() {
        panic!("Failed to connect to usbmuxd: {:?}", usbmuxd.err());
    }
    let mut usbmuxd = usbmuxd.unwrap();

    let devs = usbmuxd.get_devices().await.unwrap();
    if devs.is_empty() {
        panic!("No devices found");
    }

    let provider = devs
        .iter()
        .next()
        .unwrap()
        .to_provider(UsbmuxdAddr::from_env_var().unwrap(), "isideload-demo");

    let sideload_config =
        SideloadConfiguration::builder().team_selection(TeamSelection::Prompt(|teams| {
            println!("Please select a team:");
            for (index, team) in teams.iter().enumerate() {
                println!(
                    "{}: {} ({})",
                    index + 1,
                    team.name.as_deref().unwrap_or("<Unnamed>"),
                    team.team_id
                );
            }
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            let selection = input.trim().parse::<usize>().ok()?;
            if selection == 0 || selection > teams.len() {
                return None;
            }
            Some(teams[selection - 1].team_id.clone())
        }));

    let result = sideload_app(&provider, &mut dev_session, app_path, &sideload_config).await;
    match result {
        Ok(_) => println!("App installed successfully"),
        Err(e) => panic!("Failed to install app: {:?}", e),
    }
}
