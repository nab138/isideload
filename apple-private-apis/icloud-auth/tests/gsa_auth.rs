#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use icloud_auth::*;
    use omnisette::AnisetteConfiguration;

    #[tokio::test]
    async fn gsa_auth() {
        println!("gsa auth test");
        let email = std::env::var("apple_email").unwrap_or_else(|_| {
            println!("Enter Apple email: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            input.trim().to_string()
        });

        let password = std::env::var("apple_password").unwrap_or_else(|_| {
            println!("Enter Apple password: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            input.trim().to_string()
        });

        let appleid_closure = move || Ok((email.clone(), password.clone()));
        // ask console for 2fa code, make sure it is only 6 digits, no extra characters
        let tfa_closure = || {
            println!("Enter 2FA code: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            Ok(input.trim().to_string())
        };
        let acc = AppleAccount::login(
            appleid_closure,
            tfa_closure,
            AnisetteConfiguration::new()
                .set_configuration_path(PathBuf::from_str("anisette_test").unwrap()),
        )
        .await;

        let account = acc.unwrap();
        println!("data {:?}", account.get_name());
        println!("PET: {}", account.get_pet().unwrap());
        return;
    }
}
