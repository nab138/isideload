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
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperDevice {
    pub name: String,
    pub device_id: String,
    pub device_number: String,
    pub status: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListDevicesResponse {
    pub devices: Vec<DeveloperDevice>,
    pub result_code: i64,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentCertificate {
    pub name: String,
    pub certificate_id: String,
    pub serial_number: Option<String>,
    pub machine_id: Option<String>,
    pub cert_content: Option<ByteBuf>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListCertificatesResponse {
    pub certificates: Vec<DevelopmentCertificate>,
    pub result_code: i64,
}
