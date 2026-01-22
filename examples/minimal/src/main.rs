use std::{env, path::PathBuf};

use isideload::{
    anisette::remote_v3::RemoteV3AnisetteProvider, auth::apple_account::AppleAccountBuilder,
};

#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let _app_path = PathBuf::from(
        args.get(1)
            .expect("Please provide the path to the app to install"),
    );
    let apple_id = args
        .get(2)
        .expect("Please provide the Apple ID to use for installation");
    let apple_password = args.get(3).expect("Please provide the Apple ID password");

    let get_2fa_code = || {
        let mut code = String::new();
        println!("Enter 2FA code:");
        std::io::stdin().read_line(&mut code).unwrap();
        Ok(code.trim().to_string())
    };

    let _account = AppleAccountBuilder::new(apple_id)
        .danger_debug(true)
        .anisette(RemoteV3AnisetteProvider::default().set_serial_number("2".to_string()))
        .login(apple_password, get_2fa_code)
        .await
        .unwrap();
}
