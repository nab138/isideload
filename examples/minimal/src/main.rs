use std::env;

use isideload::{
    anisette::remote_v3::RemoteV3AnisetteProvider, auth::apple_account::AppleAccountBuilder,
};
use plist_macro::pretty_print_dictionary;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    isideload::init().expect("Failed to initialize error reporting");
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
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

    let account = AppleAccountBuilder::new(apple_id)
        .anisette_provider(RemoteV3AnisetteProvider::default().set_serial_number("2".to_string()))
        .login(apple_password, get_2fa_code)
        .await;

    match &account {
        Ok(a) => println!("Logged in. {}", a),
        Err(e) => eprintln!("Failed to log in to Apple ID: {:?}", e),
    }

    let app_token = account.unwrap().get_app_token("xcode.auth").await;

    match app_token {
        Ok(t) => println!("App token: {}", pretty_print_dictionary(&t)),
        Err(e) => eprintln!("Failed to get app token: {:?}", e),
    }
}
