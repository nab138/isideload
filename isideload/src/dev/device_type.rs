use plist::{Dictionary, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
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
            // tvOS is served by the iOS service path and selects itself with the
            // `subPlatform` request field instead.
            DeveloperDeviceType::Tvos => "ios/",
            DeveloperDeviceType::Watchos => "watchos/",
        }
    }

    /// Add the request fields that select this platform.
    pub fn apply_platform_fields(&self, body: &mut Dictionary) {
        if let DeveloperDeviceType::Tvos = self {
            body.insert("subPlatform".into(), Value::String("tvOS".into()));
        }
    }
}

impl From<Option<&str>> for DeveloperDeviceType {
    /// Map a lockdown `ProductType` (e.g. `AppleTV6,2`) to a developer device type.
    ///
    /// A missing `ProductType` is treated as iOS.
    fn from(product_type: Option<&str>) -> Self {
        match product_type {
            Some(product_type) if product_type.starts_with("AppleTV") => Self::Tvos,
            Some(product_type) if product_type.starts_with("Watch") => Self::Watchos,
            _ => Self::Ios,
        }
    }
}

pub fn dev_url(endpoint: &str, device_type: impl Into<Option<DeveloperDeviceType>>) -> String {
    format!(
        "https://developerservices2.apple.com/services/QH65B2/{}{}.action?clientId=XABBG36SBA",
        device_type
            .into()
            .unwrap_or(DeveloperDeviceType::Ios)
            .url_segment(),
        endpoint,
    )
}
