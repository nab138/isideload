use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperTeam {
    name: String,
    team_id: String,
    r#type: String,
    status: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListTeamResponse {
    pub teams: Vec<DeveloperTeam>,
    pub result_code: i64,
}
