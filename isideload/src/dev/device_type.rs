#[derive(Debug, Clone)]
pub enum DeveloperDeviceType {
    Any,
    Ios,
    Tvos,
    Watchos,
}

impl DeveloperDeviceType {
    pub fn url_segment(&self) -> &'static str {
        match self {
            DeveloperDeviceType::Any => "",
            DeveloperDeviceType::Ios => "ios/",
            DeveloperDeviceType::Tvos => "tvos/",
            DeveloperDeviceType::Watchos => "watchos/",
        }
    }

    pub fn dev_url(&self, endpoint: &str) -> String {
        format!(
            "https://developerservices2.apple.com/services/QH65B2/{}{}.action?clientId=XABBG36SBA",
            self.url_segment(),
            endpoint,
        )
    }
}
