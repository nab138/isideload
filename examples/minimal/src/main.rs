use std::env;

use isideload::{
    anisette::remote_v3::RemoteV3AnisetteProvider, auth::apple_account::AppleAccount,
    dev::developer_session::DeveloperSession,
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
    // let _app_path = PathBuf::from(
    //     args.get(1)
    //         .expect("Please provide the path to the app to install"),
    // );
    let apple_id = args
        .get(1)
        .expect("Please provide the Apple ID to use for installation");
    let apple_password = args.get(2).expect("Please provide the Apple ID password");

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

    let dev_session = DeveloperSession::from_account(&mut account)
        .await
        .expect("Failed to create developer session");

    let teams = dev_session
        .list_teams()
        .await
        .expect("Failed to list teams");

    let team = teams
        .get(0)
        .expect("No developer teams available for this account");

    let res = dev_session
        .revoke_development_cert(team, "2655CFC31A258B1B4D7D9FC22E23AEC3", None)
        .await
        .expect("Failed to list developer devices");

    println!("{:?}", res);
}
