use crate::entities_v2::user::MentorFeedbackStyle;

/// Shared policy for automatic feedbacks and all interactive mentor entry points.
pub fn with_mentor_feedback_style(task: &str, style: MentorFeedbackStyle) -> String {
    let (name, guidance) = match style {
        MentorFeedbackStyle::Balanced => (
            "balanced",
            "Offer precise, proportionate observations. Recognize strengths and identify weaknesses when warranted; neither praise nor criticism is mandatory. Choose what is most useful in this material.",
        ),
        MentorFeedbackStyle::Challenging => (
            "challenging",
            "Take a more probing, demanding stance: test assumptions, surface supported contradictions and weak decisions, or challenge ineffective literary choices. Explain why and suggest concrete alternatives when useful. Be candid, not harsh, contrarian or diagnostic.",
        ),
        MentorFeedbackStyle::Companionable => (
            "companionable",
            "Be conversational, exploratory and less directive. Prefer specific observations, connections and useful questions to prescribed plans. Still name important weaknesses honestly; companionship is not automatic agreement or reassurance.",
        ),
    };
    format!(
        "{task}\n\n{}\n\nSelected engagement style: {name}.\n{guidance}\n",
        include_str!("mentor_style.md")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mentor_style_preserves_candour_genre_and_request_precedence() {
        for style in [
            MentorFeedbackStyle::Balanced,
            MentorFeedbackStyle::Challenging,
            MentorFeedbackStyle::Companionable,
        ] {
            let prompt = with_mentor_feedback_style("Return the task's JSON contract.", style);
            assert!(prompt.starts_with("Return the task's JSON contract."));
            assert!(prompt.contains("Do not default to praise"));
            assert!(prompt.contains(
                "Literary, humorous and observational writing can deserve direct criticism"
            ));
            assert!(prompt.contains("explicit current request takes precedence"));
            assert!(prompt.contains("do not impose a quota of criticism"));
            assert!(prompt.contains(&format!(
                "Selected engagement style: {}.",
                style.to_db().to_ascii_lowercase()
            )));
        }
    }
}
