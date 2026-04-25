use crate::domain::IntentProfile;

#[derive(Debug, Clone)]
pub struct IntentCatalog {
    profiles: Vec<IntentProfile>,
}

impl IntentCatalog {
    pub fn default_catalog() -> Self {
        Self {
            profiles: vec![
                investigate(),
                slack_to_pr(),
                presentation(),
                design_to_prs(),
                brainstorm(),
                review_pr(),
                address_pr_comments(),
            ],
        }
    }

    pub fn find(&self, id: &str) -> Option<IntentProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
    }

    pub fn available_ids(&self) -> Vec<String> {
        self.profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect()
    }

    pub fn profiles(&self) -> &[IntentProfile] {
        &self.profiles
    }
}

fn investigate() -> IntentProfile {
    IntentProfile {
        id: "investigate".to_string(),
        name: "Investigate".to_string(),
        summary: "Answer a technical question with evidence across repos and sources.".to_string(),
        skill_weights: vec!["evidence-skill".to_string()],
        mcp_weights: vec![
            "slack".to_string(),
            "notion".to_string(),
            "jira".to_string(),
        ],
        instructions: vec![
            "Investigate before answering.".to_string(),
            "Prefer evidence over guesses.".to_string(),
            "Cite files, commits, docs, tickets, or messages when available.".to_string(),
            "Keep caveats visible.".to_string(),
            "Write a concise answer the manager can use.".to_string(),
        ],
    }
}

fn slack_to_pr() -> IntentProfile {
    IntentProfile {
        id: "slack-to-pr".to_string(),
        name: "Slack to Jira to PR".to_string(),
        summary: "Turn discussion into tracked implementation work.".to_string(),
        skill_weights: vec!["implementation-skill".to_string()],
        mcp_weights: vec![
            "slack".to_string(),
            "jira".to_string(),
            "github".to_string(),
        ],
        instructions: vec![
            "Preserve the original request and decisions.".to_string(),
            "Keep work, branch, and PR context aligned.".to_string(),
        ],
    }
}

fn presentation() -> IntentProfile {
    IntentProfile {
        id: "presentation".to_string(),
        name: "Technical Presentation".to_string(),
        summary: "Prepare a technical narrative and supporting artifacts.".to_string(),
        skill_weights: vec!["presentation-skill".to_string()],
        mcp_weights: vec!["notion".to_string(), "github".to_string()],
        instructions: vec![
            "Separate audience needs from implementation detail.".to_string(),
            "Keep claims traceable to evidence.".to_string(),
        ],
    }
}

fn design_to_prs() -> IntentProfile {
    IntentProfile {
        id: "design-to-prs".to_string(),
        name: "Design Doc to PRs".to_string(),
        summary: "Move from design context into reviewable implementation slices.".to_string(),
        skill_weights: vec![
            "design-skill".to_string(),
            "implementation-skill".to_string(),
        ],
        mcp_weights: vec!["notion".to_string(), "github".to_string()],
        instructions: vec![
            "Keep design decisions attached to implementation work.".to_string(),
            "Prefer reviewable PR slices.".to_string(),
        ],
    }
}

fn brainstorm() -> IntentProfile {
    IntentProfile {
        id: "brainstorm".to_string(),
        name: "Brainstorm".to_string(),
        summary: "Explore an idea without committing to implementation.".to_string(),
        skill_weights: vec!["brainstorming-skill".to_string()],
        mcp_weights: Vec::new(),
        instructions: vec![
            "Clarify the idea before producing artifacts.".to_string(),
            "Keep options and trade-offs visible.".to_string(),
        ],
    }
}

fn review_pr() -> IntentProfile {
    IntentProfile {
        id: "review-pr".to_string(),
        name: "Review Peer PR".to_string(),
        summary: "Review a peer pull request from local context.".to_string(),
        skill_weights: vec!["review-skill".to_string()],
        mcp_weights: vec!["github".to_string(), "git".to_string()],
        instructions: vec![
            "Prioritize correctness, regressions, and missing tests.".to_string(),
            "Keep review comments specific and actionable.".to_string(),
        ],
    }
}

fn address_pr_comments() -> IntentProfile {
    IntentProfile {
        id: "address-pr-comments".to_string(),
        name: "Address PR Comments".to_string(),
        summary: "Resolve and discuss comments on owned pull requests.".to_string(),
        skill_weights: vec!["review-response-skill".to_string()],
        mcp_weights: vec!["github".to_string(), "git".to_string()],
        instructions: vec![
            "Separate accepted changes from discussion points.".to_string(),
            "Preserve reviewer context.".to_string(),
        ],
    }
}
