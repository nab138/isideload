pub mod auth;

use idevice::{plist, pretty_print_plist};

pub fn test() -> () {
    println!(
        "{}",
        pretty_print_plist(&plist!({
            "code": "hello"
        }))
    );
}
