pub fn title_from_goal(goal: &str) -> String {
    let clean_goal = collapse_spaces(goal.trim().trim_end_matches('.'));
    let story_body = strip_user_story_prefix(&clean_goal);

    match story_body {
        Some(body) => title_from_story_body(body),
        None => clean_goal,
    }
}

pub fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "work".to_string()
    } else {
        slug
    }
}

fn title_from_story_body(body: &str) -> String {
    let words: Vec<String> = body
        .split_whitespace()
        .map(clean_word)
        .filter(|word| !word.is_empty())
        .filter(|word| !is_story_filler(word))
        .take(5)
        .collect();

    if words.is_empty() {
        "Work".to_string()
    } else {
        sentence_case(&words.join(" "))
    }
}

fn strip_user_story_prefix(value: &str) -> Option<&str> {
    let lower = value.to_ascii_lowercase();
    for prefix in [
        "as se, i want to ",
        "as a se, i want to ",
        "as an se, i want to ",
        "as a software engineer, i want to ",
        "as an engineer, i want to ",
        "i want to ",
    ] {
        if lower.starts_with(prefix) {
            return value.get(prefix.len()..);
        }
    }
    None
}

fn clean_word(word: &str) -> String {
    word.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_story_filler(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "the"
            | "for"
            | "my"
            | "that"
            | "needs"
            | "need"
            | "across"
            | "one"
            | "or"
            | "more"
            | "repo"
            | "repos"
            | "repository"
            | "repositories"
            | "investigation"
            | "investigate"
    )
}

fn sentence_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

fn collapse_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{slugify, title_from_goal};

    #[test]
    fn builds_short_titles_from_user_stories() {
        let title = title_from_goal(
            "As SE, I want to answer a technical question for my manager that needs investigation across one or more repositories.",
        );

        assert_eq!(title, "Answer technical question manager");
    }

    #[test]
    fn keeps_short_plain_goals_readable() {
        assert_eq!(
            title_from_goal("Investigate billing timeout"),
            "Investigate billing timeout"
        );
    }

    #[test]
    fn slugifies_titles() {
        assert_eq!(
            slugify("Answer: Billing Question!"),
            "answer-billing-question"
        );
    }
}
