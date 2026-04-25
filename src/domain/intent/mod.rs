mod catalog;

pub use catalog::IntentCatalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentProfile {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub skill_weights: Vec<String>,
    pub mcp_weights: Vec<String>,
    pub instructions: Vec<String>,
}
