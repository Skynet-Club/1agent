//! Partner voice modes.
//!
//! The base prompt stays the same; a voice block changes how the partner
//! speaks, not what counts as evidence. `operator` is the default. `advisor`
//! adds the tradeoff that can change the recommendation. `companion` adds one
//! sentence of context and warmth, without flattery or role-play.

use crate::user_model::VoiceMode;

pub const OPERATOR: &str = "VOICE: operator. Execute. Lead with the result and the next action. Keep warmth out of the way: no greetings, no praise, no role-play.";
pub const ADVISOR: &str = "VOICE: advisor. Give the recommendation first, then the tradeoff that matters and what would change it. Do not hide uncertainty behind confidence. No flattery, no role-play.";
pub const COMPANION: &str = "VOICE: companion. Be direct and warm: acknowledge the user's context in one sentence, then give the result or next action. No flattery, no role-play, no emotional padding, no pretending to feel.";

pub fn instruction(mode: VoiceMode) -> &'static str {
    match mode {
        VoiceMode::Operator => OPERATOR,
        VoiceMode::Advisor => ADVISOR,
        VoiceMode::Companion => COMPANION,
    }
}

pub fn with_voice(base: &str, mode: VoiceMode) -> String {
    format!("{base}\n\nVOICE MODE\n{}", instruction(mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "You are 1-Agent. Result first. Ask at most one question.";

    #[test]
    fn each_mode_has_a_distinct_instruction() {
        assert!(OPERATOR.contains("operator"));
        assert!(ADVISOR.contains("advisor"));
        assert!(COMPANION.contains("companion"));
        assert_ne!(
            instruction(VoiceMode::Operator),
            instruction(VoiceMode::Advisor)
        );
        assert_ne!(
            instruction(VoiceMode::Advisor),
            instruction(VoiceMode::Companion)
        );
    }

    #[test]
    fn advisor_keeps_the_tradeoff_and_flip_condition() {
        let prompt = with_voice(BASE, VoiceMode::Advisor);
        assert!(prompt.contains(BASE));
        assert!(prompt.contains("tradeoff"));
        assert!(prompt.contains("what would change it"));
    }

    #[test]
    fn companion_is_warm_but_not_sycophantic_or_role_playing() {
        let prompt = with_voice(BASE, VoiceMode::Companion);
        assert!(prompt.contains("direct and warm"));
        assert!(prompt.contains("No flattery"));
        for forbidden in ["收到", "遵命", "保证完成任务", "praise the user"] {
            assert!(!prompt.contains(forbidden));
        }
    }

    #[test]
    fn operator_leads_with_result_and_next_action() {
        let prompt = with_voice(BASE, VoiceMode::Operator);
        assert!(prompt.contains("Lead with the result and the next action"));
        assert!(prompt.contains(BASE));
    }
}
