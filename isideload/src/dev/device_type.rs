use plist::{Dictionary, Value};

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
            DeveloperDeviceType::Watchos => "ios/",
        }
    }
}

pub fn apply_platform_to_body(
    body: &mut Dictionary,
    device_type: &Option<DeveloperDeviceType>,
) {
    if let Some(DeveloperDeviceType::Watchos) = device_type.as_ref() {
        body.insert(
            "DTDK_Platform".to_string(),
            Value::String("watchos".to_string()),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchos_uses_ios_developer_service_path() {
        assert_eq!(
            dev_url(
                "listDevices",
                Some(DeveloperDeviceType::Watchos),
            ),
            "https://developerservices2.apple.com/services/QH65B2/ios/listDevices.action?clientId=XABBG36SBA"
        );
    }

    #[test]
    fn watchos_sets_platform_marker() {
        let mut body = Dictionary::new();

        apply_platform_to_body(
            &mut body,
            &Some(DeveloperDeviceType::Watchos),
        );

        assert_eq!(
            body.get("DTDK_Platform").and_then(Value::as_string),
            Some("watchos")
        );
    }
}
