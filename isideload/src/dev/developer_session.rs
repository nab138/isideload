use std::sync::Arc;

use crate::auth::apple_account::AppleAccount;

struct DeveloperSession {
    apple_account: Arc<AppleAccount>,
}

impl DeveloperSession {
    pub fn new(apple_account: Arc<AppleAccount>) -> Self {
        DeveloperSession { apple_account }
    }
}
