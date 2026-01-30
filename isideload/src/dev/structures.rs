use serde::Deserialize;
use serde_bytes::ByteBuf;

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
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperTeam {
    pub name: Option<String>,
    pub team_id: String,
    pub r#type: Option<String>,
    pub status: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListTeamsResponse {
    pub teams: Vec<DeveloperTeam>,
    pub result_code: i64,
    pub result_string: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperDevice {
    pub name: String,
    pub device_id: String,
    pub device_number: String,
    pub status: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentCertificate {
    pub name: String,
    pub certificate_id: String,
    pub serial_number: Option<String>,
    pub machine_id: Option<String>,
    pub cert_content: Option<ByteBuf>,
}

// the automatic debug implementation spams the console with the cert content bytes
impl std::fmt::Debug for DevelopmentCertificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DevelopmentCertificate")
            .field("name", &self.name)
            .field("certificate_id", &self.certificate_id)
            .field("serial_number", &self.serial_number)
            .field("machine_id", &self.machine_id)
            .field(
                "cert_content",
                &self
                    .cert_content
                    .as_ref()
                    .map(|c| format!("Some([{} bytes])", c.len()))
                    .unwrap_or("None".to_string()),
            )
            .finish()
    }
}
