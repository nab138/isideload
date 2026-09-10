//! Minimal harness for the GSA 503 fix: logs in and then asks for the `xcode.auth` app token,
//! which is exactly the init -> complete -> apptokens sequence that Apple's edge was killing.
//! Deliberately stops before touching a device, so it can be run with nothing plugged in.
//!
//!     cargo run -p login-check -- <apple-id> <password>
//!
//! Omit the password for a credential-free smoke test that only fetches the GrandSlam URL bag,
//! which is enough to tell whether Apple's edge accepts the User-Agent at all:
//!
//!     cargo run -p login-check -- <apple-id>
//!
//! Set RUST_LOG=isideload=debug for the per-request logging.

use isideload::{
    anisette::remote_v3::RemoteV3AnisetteProvider,
    auth::apple_account::{AppleAccount, TwoFactorCallbackParams, TwoFactorCallbackResponse},
    util::storage::InMemoryStorage,
};
use std::env;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    isideload::init().expect("Failed to initialize error reporting");
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("isideload=debug")),
        )
        .init();

    let args: Vec<String> = env::args().collect();

    // Anisette provisioning happens before any credential is used, and it drives GrandSlam
    // requests of its own (midStartProvisioning / midFinishProvisioning) through exactly the
    // same headers as login. Isolating it makes it testable without an Apple ID, and the
    // in-memory storage forces a fresh provisioning on every run instead of reusing a cached
    // state from the keyring.
    if args.get(1).map(String::as_str) == Some("--provision") {
        println!("--- anisette provisioning only (no credentials) ---");
        let mut account = AppleAccount::builder("provision-only@example.com")
            .anisette_provider(
                RemoteV3AnisetteProvider::default()
                    .unwrap()
                    .set_storage(Box::new(InMemoryStorage::new()))
                    .set_serial_number("2".to_string()),
            )
            .build()
            .await
            .expect("failed to build account");

        let grandslam = account.grandslam_client.clone();
        match account
            .anisette_generator
            .get_anisette_data(grandslam)
            .await
        {
            Ok(_) => println!("--- PROVISIONING OK ---"),
            Err(report) => {
                eprintln!("PROVISIONING FAILED:\n{report:?}");
                std::process::exit(1);
            }
        }
        return;
    }

    let apple_id = args
        .get(1)
        .expect("usage: login-check <apple-id> [password] | login-check --provision");
    let password = args.get(2);

    // No password: just build the account, which performs the GrandSlam URL bag request. That
    // request goes to gsa.apple.com with the same headers as the login requests, so it fails
    // the same way if the User-Agent is rejected - but it needs no credentials.
    if password.is_none() {
        println!("--- no password given: URL bag smoke test only ---");
        match AppleAccount::builder(apple_id)
            .anisette_provider(
                RemoteV3AnisetteProvider::default()
                    .unwrap()
                    .set_serial_number("2".to_string()),
            )
            .build()
            .await
        {
            Ok(_) => {
                println!("--- URL bag OK: gsa.apple.com accepted our headers ---");
                return;
            }
            Err(report) => {
                eprintln!("URL BAG FAILED:\n{report:?}");
                std::process::exit(1);
            }
        }
    }
    let password = password.unwrap();

    let two_factor = async |params: TwoFactorCallbackParams| {
        if params.unknown {
            println!("The last 2FA method failed, try another one.");
        } else if params.sms {
            println!("Enter the 2FA code sent by SMS:");
        } else {
            println!("Enter the 2FA code sent to your devices:");
        }

        let mut code = String::new();
        std::io::stdin().read_line(&mut code).unwrap();
        Ok(TwoFactorCallbackResponse::SubmitCode(
            code.trim().to_string(),
        ))
    };

    println!("--- step 1+2: init / complete ---");
    let mut account = match AppleAccount::builder(apple_id)
        .anisette_provider(
            RemoteV3AnisetteProvider::default()
                .unwrap()
                .set_serial_number("2".to_string()),
        )
        .login(password, two_factor)
        .await
    {
        Ok(account) => account,
        Err(report) => {
            eprintln!("LOGIN FAILED (init/complete):\n{report:?}");
            std::process::exit(1);
        }
    };
    println!("--- login OK ---");

    println!("--- step 3: apptokens (this is the request that used to 503) ---");
    match account.get_app_token("xcode.auth").await {
        Ok(_) => println!("--- apptokens OK: got an xcode.auth token, the 503 is fixed ---"),
        Err(report) => {
            eprintln!("APPTOKENS FAILED:\n{report:?}");
            std::process::exit(1);
        }
    }
}
