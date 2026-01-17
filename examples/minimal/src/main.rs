use std::{env, path::PathBuf};

use isideload::auth::apple_account::AppleAccountBuilder;

fn main() {
    let args: Vec<String> = env::args().collect();
    let app_path = PathBuf::from(
        args.get(1)
            .expect("Please provide the path to the app to install"),
    );
    let apple_id = args
        .get(2)
        .expect("Please provide the Apple ID to use for installation");
    let apple_password = args.get(3).expect("Please provide the Apple ID password");

    let account = AppleAccountBuilder::new(apple_id)
        .danger_debug(true)
        .build()
        .unwrap();
}
