use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestCase {
    pub input: String,
    pub output: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProblemPayload {
    pub name: String,
    pub group: String,
    pub url: String,
    pub time_limit: u64,
    pub memory_limit: u64,
    pub tests: Vec<TestCase>,
}
