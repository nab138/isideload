pub mod app_groups;
pub mod app_ids;
pub mod certificates;
pub mod developer_session;
pub mod device_type;
pub mod devices;
pub mod teams;

// some non-alphanumeric characters cause Developer error 35: An invalid value was provided for the parameter 'appIdName'.
pub fn normalize_app_names(name: &str) -> String {
    let normalized: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if normalized.is_empty() {
        "App".to_string()
    } else {
        normalized
    }
}
