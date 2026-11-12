//! Partner voice modes and task forms.
//!
//! The base prompt stays the same; a voice block changes how the partner
//! speaks, not what counts as evidence. `operator` is the default. `advisor`
//! adds the tradeoff that can change the recommendation. `companion` adds one
//! sentence of context and warmth, without flattery or role-play.
//!
//! A task form is chosen from the cognitive mode: decision tasks lead with a
//! recommendation and a flip condition, research tasks separate evidence from
//! inference, and execution tasks lead with what changed and how it was
//! verified.

use crate::domain::CognitiveMode;
use crate::user_model::VoiceMode;

pub const OPERATOR: &str = "VOICE: operator. Execute. Lead with the result and the next action. Keep warmth out of the way: no greetings, no praise, no role-play.";
pub const ADVISOR: &str = "VOICE: advisor. Give the recommendation first, then the tradeoff that matters and what would change it. Do not hide uncertainty behind confidence. No flattery, no role-play.";
pub const COMPANION: &str = "VOICE: companion. Be direct and warm: acknowledge the user's context in one sentence, then give the result or next action. No flattery, no role-play, no emotional padding, no pretending to feel.";

pub const DECISION_FORM: &str = "FORM: decision. Lead with the recommendation. Name the decisive tradeoff, the confidence you can defend, and the observation that would flip it. Do not bury the decision under research.";
pub const RESEARCH_FORM: &str = "FORM: research. Separate observed evidence, inference and unknown. Cite the source for each material claim. State what would falsify the conclusion.";
pub const EXECUTION_FORM: &str = "FORM: execution. Lead with what changed and the next action. Report verification and failures plainly. Never claim an action without a receipt.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceForm {
    Decision,
    Research,
    Execution,
}

impl VoiceForm {
    pub fn for_task(mode: CognitiveMode) -> Self {
        match mode {
            CognitiveMode::DecisionAnalysis | CognitiveMode::Escalate => Self::Decision,
            CognitiveMode::EvidenceResearch | CognitiveMode::Diagnosis => Self::Research,
            CognitiveMode::DirectAnswer | CognitiveMode::Execute => Self::Execution,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Research => "research",
            Self::Execution => "execution",
        }
    }
}

pub fn instruction(mode: VoiceMode) -> &'static str {
    match mode {
        VoiceMode::Operator => OPERATOR,
        VoiceMode::Advisor => ADVISOR,
        VoiceMode::Companion => COMPANION,
    }
}

pub fn form_instruction(form: VoiceForm) -> &'static str {
    match form {
        VoiceForm::Decision => DECISION_FORM,
        VoiceForm::Research => RESEARCH_FORM,
        VoiceForm::Execution => EXECUTION_FORM,
    }
}

pub fn with_voice_form(base: &str, mode: VoiceMode, form: VoiceForm) -> String {
    format!(
        "{base}\n\nVOICE MODE\n{}\n\nVOICE FORM\n{}",
        instruction(mode),
        form_instruction(form)
    )
}

pub fn with_voice(base: &str, mode: VoiceMode) -> String {
    with_voice_form(base, mode, VoiceForm::Execution)
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

    #[test]
    fn task_forms_follow_the_cognitive_mode() {
        assert_eq!(
            VoiceForm::for_task(CognitiveMode::DecisionAnalysis),
            VoiceForm::Decision
        );
        assert_eq!(
            VoiceForm::for_task(CognitiveMode::Escalate),
            VoiceForm::Decision
        );
        assert_eq!(
            VoiceForm::for_task(CognitiveMode::EvidenceResearch),
            VoiceForm::Research
        );
        assert_eq!(
            VoiceForm::for_task(CognitiveMode::Diagnosis),
            VoiceForm::Research
        );
        assert_eq!(
            VoiceForm::for_task(CognitiveMode::Execute),
            VoiceForm::Execution
        );
        assert_eq!(
            VoiceForm::for_task(CognitiveMode::DirectAnswer),
            VoiceForm::Execution
        );
        assert_ne!(DECISION_FORM, RESEARCH_FORM);
        assert_ne!(RESEARCH_FORM, EXECUTION_FORM);
        assert_eq!(VoiceForm::Decision.as_str(), "decision");
    }

    #[test]
    fn form_block_is_appended_without_changing_the_base_or_voice() {
        let prompt = with_voice_form(BASE, VoiceMode::Advisor, VoiceForm::Decision);
        assert!(prompt.contains(BASE));
        assert!(prompt.contains(ADVISOR));
        assert!(prompt.contains("FORM: decision"));
        assert!(prompt.contains("recommendation"));
        assert!(prompt.contains("flip"));
        let research = with_voice_form(BASE, VoiceMode::Operator, VoiceForm::Research);
        assert!(research.contains("observed evidence"));
        assert!(research.contains("falsify"));
        let execution = with_voice_form(BASE, VoiceMode::Companion, VoiceForm::Execution);
        assert!(execution.contains("what changed"));
        assert!(execution.contains("receipt"));
        assert!(execution.contains(COMPANION));
    }
}
