//! Requirement regression: is this request a goal, or a means to one?
//!
//! The type here is the auditable half of the capability. It carries no model
//! behaviour; it only decides whether a proposal is well formed and whether the
//! stop rule was actually satisfied. Everything a model produces must pass
//! [`ReframeProposal::validate`] before it can appear in a journal, a receipt or
//! a transcript.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::safety::contains_disallowed_control;

pub const REFRAME_SCHEMA_VERSION: u32 = 1;

/// The prompt used for every reframe call, shared verbatim with the research
/// harness (`scripts/reframing/run_pilot.py` reads the same file). The held-out
/// measurements were made with exactly these bytes; a test pins the hash.
pub const REFRAME_SYSTEM_PROMPT: &str = include_str!("../prompts/reframe-v3.txt");

/// SHA-256 of [`REFRAME_SYSTEM_PROMPT`] at the time the held-out rounds ran.
pub const REFRAME_PROMPT_SHA256: &str =
    "650588186c30dffed4ccc6bf2a9e7e601f8871eff7b2e84a9d7f80aecb069e16";
pub const MAX_OPTIONS: usize = 3;
pub const MIN_OPTIONS: usize = 2;
pub const MAX_HYPOTHESES: usize = 3;
pub const MAX_SUBSTITUTES: usize = 3;
pub const MAX_TEXT_CHARS: usize = 2_000;

/// Whether the request already states the outcome, or names something that only
/// exists to reach one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestRole {
    End,
    Means,
}

impl RequestRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::End => "end",
            Self::Means => "means",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::End => "目的",
            Self::Means => "手段",
        }
    }
}

/// The three conditions that must all hold before regressing one more level.
///
/// A model may claim these; the caller is responsible for the evidence behind
/// them. This type only makes the claim explicit and checkable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopRule {
    pub verifiable: bool,
    pub deliverable: bool,
    pub better_substitute_exists: bool,
}

/// Whether the three stop-rule conditions were asserted by a caller or derived
/// from what the model returned. A derived rule is weaker evidence and must be
/// readable as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StopRuleSource {
    #[default]
    Asserted,
    Derived,
}

impl StopRule {
    pub fn passes(&self) -> bool {
        self.verifiable && self.deliverable && self.better_substitute_exists
    }
}

/// One proposed regress: what was asked, what it may be for, and the question
/// that would settle it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReframeProposal {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// The user's words, verbatim. Never rewritten.
    pub stated_request: String,
    pub classification: RequestRole,
    #[serde(default)]
    pub confidence_bps: Option<u16>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub hypotheses: Vec<String>,
    #[serde(default)]
    pub substitutes: Vec<String>,
    #[serde(default)]
    pub stop_rule: StopRule,
    #[serde(default)]
    pub stop_rule_source: StopRuleSource,
}

fn default_schema_version() -> u32 {
    REFRAME_SCHEMA_VERSION
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReframeError {
    #[error("unsupported reframe schema version {0}")]
    UnsupportedVersion(u32),
    #[error("stated request is empty, unsafe or larger than {MAX_TEXT_CHARS} characters")]
    InvalidStatedRequest,
    #[error("an end request must not carry a question or options")]
    EndCarriesQuestion,
    #[error("an end request must not carry substitutes")]
    EndCarriesSubstitute,
    #[error("a means request must carry exactly one question")]
    MeansWithoutQuestion,
    #[error("question is empty, unsafe or larger than {MAX_TEXT_CHARS} characters")]
    InvalidQuestion,
    #[error("a means request needs {MIN_OPTIONS} to {MAX_OPTIONS} options, got {0}")]
    InvalidOptionCount(usize),
    #[error("option {0} is empty, duplicated, unsafe or too large")]
    InvalidOption(usize),
    #[error("a two-option question must not be a yes/no question")]
    YesNoQuestion,
    #[error("more than {MAX_HYPOTHESES} hypotheses")]
    TooManyHypotheses,
    #[error("hypothesis {0} is empty, unsafe or too large")]
    InvalidHypothesis(usize),
    #[error("more than {MAX_SUBSTITUTES} substitutes")]
    TooManySubstitutes,
    #[error("substitute {0} is empty, unsafe or too large")]
    InvalidSubstitute(usize),
    #[error("a means request requires all three stop-rule conditions")]
    StopRuleNotSatisfied,
    #[error("confidence {0} exceeds 10000 basis points")]
    InvalidConfidence(u16),
    #[error("cannot accept a proposal that asked no question")]
    AcceptWithoutQuestion,
    #[error("option index {0} is outside the proposed options")]
    OptionOutOfRange(usize),
    #[error("substitute index {0} is outside the proposed substitutes")]
    SubstituteOutOfRange(usize),
    #[error("model output is not a valid reframe object: {0}")]
    ModelOutputInvalid(String),
    #[error("model classification '{0}' is neither means nor end")]
    ModelClassificationInvalid(String),
}

/// The shape a model returns. It is deliberately not [`ReframeProposal`]: the
/// model does not know the verbatim request and must not assert the stop rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelProposal {
    pub classification: String,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub hypotheses: Vec<String>,
    #[serde(default)]
    pub substitutes: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

impl ModelProposal {
    /// Converts a model answer into a proposal, deriving what the model cannot
    /// know. An `end` keeps no question, options or substitutes.
    pub fn into_proposal(
        self,
        stated_request: impl Into<String>,
    ) -> Result<ReframeProposal, ReframeError> {
        let normalized = self.classification.trim().to_ascii_lowercase();
        let classification = match normalized.as_str() {
            "means" => RequestRole::Means,
            "end" => RequestRole::End,
            other => return Err(ReframeError::ModelClassificationInvalid(other.into())),
        };
        let has_substitutes = !self.substitutes.is_empty();
        let confidence_bps = self
            .confidence
            .map(|value| (value.clamp(0.0, 1.0) * 10_000.0).round() as u16);
        let mut proposal = ReframeProposal {
            schema_version: REFRAME_SCHEMA_VERSION,
            stated_request: stated_request.into(),
            classification,
            confidence_bps,
            question: self.question,
            options: self.options,
            hypotheses: self.hypotheses,
            substitutes: self.substitutes,
            stop_rule: StopRule {
                verifiable: true,
                deliverable: has_substitutes,
                better_substitute_exists: has_substitutes,
            },
            stop_rule_source: StopRuleSource::Derived,
        };
        if classification == RequestRole::End {
            proposal.question = None;
            proposal.options.clear();
            proposal.substitutes.clear();
        }
        Ok(proposal)
    }
}

/// Parses model text into a [`ModelProposal`], tolerating a markdown fence.
pub fn parse_model_output(text: &str) -> Result<ModelProposal, ReframeError> {
    let trimmed = text.trim();
    let body = if let Some(rest) = trimmed.strip_prefix("```") {
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        rest.trim_end_matches("```").trim()
    } else {
        trimmed
    };
    serde_json::from_str(body).map_err(|error| ReframeError::ModelOutputInvalid(error.to_string()))
}

/// The user message sent alongside [`REFRAME_SYSTEM_PROMPT`].
pub fn build_user_message(request: &str, constraints: &[String]) -> String {
    let mut out = format!("请求：{request}");
    if constraints.is_empty() {
        out.push_str("\n已知约束：无");
    } else {
        out.push_str(&format!("\n已知约束：{}", constraints.join("；")));
    }
    out
}

/// SHA-256 of the prompt, for journalling alongside a proposal.
pub fn prompt_sha256() -> String {
    hex::encode(Sha256::digest(REFRAME_SYSTEM_PROMPT.as_bytes()))
}

const YES_NO: &[&str] = &[
    "是",
    "否",
    "不是",
    "对",
    "不对",
    "会",
    "不会",
    "有",
    "没有",
    "要",
    "不要",
    "需要",
    "不需要",
    "yes",
    "no",
    "true",
    "false",
];

impl ReframeProposal {
    pub fn validate(&self) -> Result<(), ReframeError> {
        if self.schema_version != REFRAME_SCHEMA_VERSION {
            return Err(ReframeError::UnsupportedVersion(self.schema_version));
        }
        validate_text(&self.stated_request).ok_or(ReframeError::InvalidStatedRequest)?;
        if let Some(confidence) = self.confidence_bps
            && confidence > 10_000
        {
            return Err(ReframeError::InvalidConfidence(confidence));
        }

        match self.classification {
            RequestRole::End => {
                if self.question.is_some() || !self.options.is_empty() {
                    return Err(ReframeError::EndCarriesQuestion);
                }
                if !self.substitutes.is_empty() {
                    return Err(ReframeError::EndCarriesSubstitute);
                }
            }
            RequestRole::Means => {
                let question = self
                    .question
                    .as_deref()
                    .ok_or(ReframeError::MeansWithoutQuestion)?;
                validate_text(question).ok_or(ReframeError::InvalidQuestion)?;
                if !self.stop_rule.passes() {
                    return Err(ReframeError::StopRuleNotSatisfied);
                }
            }
        }

        if !self.options.is_empty() {
            if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&self.options.len()) {
                return Err(ReframeError::InvalidOptionCount(self.options.len()));
            }
            validate_list(&self.options, ReframeError::InvalidOption)?;
            if self.options.len() == 2
                && self
                    .options
                    .iter()
                    .all(|option| YES_NO.contains(&option.trim().to_lowercase().as_str()))
            {
                return Err(ReframeError::YesNoQuestion);
            }
        }

        if self.hypotheses.len() > MAX_HYPOTHESES {
            return Err(ReframeError::TooManyHypotheses);
        }
        validate_list(&self.hypotheses, ReframeError::InvalidHypothesis)?;

        if self.substitutes.len() > MAX_SUBSTITUTES {
            return Err(ReframeError::TooManySubstitutes);
        }
        validate_list(&self.substitutes, ReframeError::InvalidSubstitute)?;

        Ok(())
    }

    /// Whether this proposal actually asks the user anything.
    pub fn is_question(&self) -> bool {
        self.classification == RequestRole::Means
            && self
                .question
                .as_deref()
                .is_some_and(|q| !q.trim().is_empty())
    }

    /// Plain text for a terminal or a receipt. No formatting beyond line breaks.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("角色      {}\n", self.classification.label_zh()));
        out.push_str(&format!("原请求    {}\n", self.stated_request));
        if let Some(confidence) = self.confidence_bps {
            out.push_str(&format!("置信      {:.2}\n", confidence as f64 / 10_000.0));
        }
        if let Some(question) = &self.question {
            out.push_str(&format!("追问      {question}\n"));
        }
        if !self.options.is_empty() {
            out.push_str(&format!("选项      {}\n", self.options.join(" / ")));
        }
        if !self.hypotheses.is_empty() {
            out.push_str(&format!("可能目的  {}\n", self.hypotheses.join(" / ")));
        }
        if !self.substitutes.is_empty() {
            out.push_str(&format!("替代      {}\n", self.substitutes.join(" / ")));
        }
        out
    }
}

/// Why a user turned a proposal down.
///
/// The five classes match the labelling categories in
/// `docs/thesis/02-eval-protocol.md`, so a receipt and an annotation use the
/// same vocabulary. `PreferLiteral` is the numerator of the regret rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectedReason {
    CostTransferred,
    ConstraintUnmet,
    PreferLiteral,
    Unverifiable,
    Other,
}

impl RejectedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CostTransferred => "cost_transferred",
            Self::ConstraintUnmet => "constraint_unmet",
            Self::PreferLiteral => "prefer_literal",
            Self::Unverifiable => "unverifiable",
            Self::Other => "other",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::CostTransferred => "新增代价被转嫁",
            Self::ConstraintUnmet => "原始约束未被满足",
            Self::PreferLiteral => "我就是要原方案",
            Self::Unverifiable => "提出的目的无法验证",
            Self::Other => "其他",
        }
    }
}

/// What the user decided about a proposal. Only a human may produce this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ReframeChoice {
    Accepted {
        option_index: usize,
        #[serde(default)]
        substitute_index: Option<usize>,
    },
    Rejected {
        reason: RejectedReason,
    },
    Deferred,
}

impl ReframeChoice {
    pub fn validate_against(&self, proposal: &ReframeProposal) -> Result<(), ReframeError> {
        match self {
            Self::Accepted {
                option_index,
                substitute_index,
            } => {
                if !proposal.is_question() {
                    return Err(ReframeError::AcceptWithoutQuestion);
                }
                if *option_index >= proposal.options.len() {
                    return Err(ReframeError::OptionOutOfRange(*option_index));
                }
                if let Some(index) = substitute_index
                    && *index >= proposal.substitutes.len()
                {
                    return Err(ReframeError::SubstituteOutOfRange(*index));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn label_zh(&self) -> String {
        match self {
            Self::Accepted { option_index, .. } => format!("采纳第 {} 项", option_index + 1),
            Self::Rejected { reason } => format!("拒绝：{}", reason.label_zh()),
            Self::Deferred => "待定".into(),
        }
    }
}

/// The durable record a user can read months later. Plain text, no formatting.
pub fn render_receipt(
    proposal: &ReframeProposal,
    choice: &ReframeChoice,
    actor: &str,
    decided_at_unix_ms: u128,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("原请求    {}\n", proposal.stated_request));
    out.push_str(&format!(
        "判定      {}\n",
        proposal.classification.label_zh()
    ));
    if let Some(question) = &proposal.question {
        out.push_str(&format!("追问      {question}\n"));
    }
    if !proposal.options.is_empty() {
        out.push_str(&format!("选项      {}\n", proposal.options.join(" / ")));
    }
    if !proposal.substitutes.is_empty() {
        out.push_str(&format!("替代      {}\n", proposal.substitutes.join(" / ")));
    }
    out.push_str(&format!("决定      {}\n", choice.label_zh()));
    out.push_str(&format!("决定人    {actor}\n"));
    out.push_str(&format!("时间      {decided_at_unix_ms}\n"));
    out
}

fn validate_text(text: &str) -> Option<()> {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || text.chars().count() > MAX_TEXT_CHARS
        || contains_disallowed_control(text)
    {
        return None;
    }
    Some(())
}

fn validate_list<F>(values: &[String], error: F) -> Result<(), ReframeError>
where
    F: Fn(usize) -> ReframeError,
{
    let mut seen = std::collections::BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if validate_text(value).is_none() || !seen.insert(value.trim()) {
            return Err(error(index));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn means() -> ReframeProposal {
        ReframeProposal {
            schema_version: REFRAME_SCHEMA_VERSION,
            stated_request: "我要买个电钻".into(),
            classification: RequestRole::Means,
            confidence_bps: Some(8_200),
            question: Some("打孔是为了穿线、挂东西，还是施工改造？".into()),
            options: vec!["穿线".into(), "挂东西".into(), "施工改造".into()],
            hypotheses: vec!["联网".into(), "装书架".into()],
            substitutes: vec!["无线路由器".into()],
            stop_rule: StopRule {
                verifiable: true,
                deliverable: true,
                better_substitute_exists: true,
            },
            stop_rule_source: StopRuleSource::Asserted,
        }
    }

    #[test]
    fn valid_means_passes() {
        assert_eq!(means().validate(), Ok(()));
        assert!(means().is_question());
    }

    #[test]
    fn end_must_not_carry_a_question() {
        let mut proposal = means();
        proposal.classification = RequestRole::End;
        assert_eq!(proposal.validate(), Err(ReframeError::EndCarriesQuestion));
    }

    #[test]
    fn end_must_not_carry_substitutes() {
        let proposal = ReframeProposal {
            schema_version: REFRAME_SCHEMA_VERSION,
            stated_request: "把 typo 改掉".into(),
            classification: RequestRole::End,
            confidence_bps: None,
            question: None,
            options: vec![],
            hypotheses: vec![],
            substitutes: vec!["重写整个文件".into()],
            stop_rule: StopRule::default(),
            stop_rule_source: StopRuleSource::Asserted,
        };
        assert_eq!(proposal.validate(), Err(ReframeError::EndCarriesSubstitute));
    }

    #[test]
    fn means_without_question_fails() {
        let mut proposal = means();
        proposal.question = None;
        assert_eq!(proposal.validate(), Err(ReframeError::MeansWithoutQuestion));
    }

    #[test]
    fn means_without_stop_rule_fails() {
        let mut proposal = means();
        proposal.stop_rule.deliverable = false;
        assert_eq!(proposal.validate(), Err(ReframeError::StopRuleNotSatisfied));
    }

    #[test]
    fn two_option_yes_no_question_is_rejected() {
        let mut proposal = means();
        proposal.question = Some("需要联网吗？".into());
        proposal.options = vec!["需要".into(), "不需要".into()];
        assert_eq!(proposal.validate(), Err(ReframeError::YesNoQuestion));
    }

    #[test]
    fn duplicate_options_are_rejected() {
        let mut proposal = means();
        proposal.options = vec!["穿线".into(), "穿线".into()];
        assert_eq!(proposal.validate(), Err(ReframeError::InvalidOption(1)));
    }

    #[test]
    fn option_count_is_bounded() {
        let mut proposal = means();
        proposal.options = vec!["只有一个".into()];
        assert_eq!(
            proposal.validate(),
            Err(ReframeError::InvalidOptionCount(1))
        );
    }

    #[test]
    fn control_characters_are_rejected() {
        let mut proposal = means();
        proposal.stated_request = "买电钻\u{200b}".into();
        assert_eq!(proposal.validate(), Err(ReframeError::InvalidStatedRequest));
    }

    #[test]
    fn future_schema_version_fails_closed() {
        let mut proposal = means();
        proposal.schema_version = REFRAME_SCHEMA_VERSION + 1;
        assert_eq!(
            proposal.validate(),
            Err(ReframeError::UnsupportedVersion(REFRAME_SCHEMA_VERSION + 1))
        );
    }

    #[test]
    fn render_keeps_the_stated_request_verbatim() {
        let text = means().render();
        assert!(text.contains("我要买个电钻"));
        assert!(text.contains("手段"));
        assert!(text.contains("无线路由器"));
    }

    fn end() -> ReframeProposal {
        ReframeProposal {
            schema_version: REFRAME_SCHEMA_VERSION,
            stated_request: "把 typo 改掉".into(),
            classification: RequestRole::End,
            confidence_bps: None,
            question: None,
            options: vec![],
            hypotheses: vec![],
            substitutes: vec![],
            stop_rule: StopRule::default(),
            stop_rule_source: StopRuleSource::Asserted,
        }
    }

    #[test]
    fn accepting_an_end_proposal_is_rejected() {
        let choice = ReframeChoice::Accepted {
            option_index: 0,
            substitute_index: None,
        };
        assert_eq!(
            choice.validate_against(&end()),
            Err(ReframeError::AcceptWithoutQuestion)
        );
    }

    #[test]
    fn option_index_must_be_in_range() {
        let choice = ReframeChoice::Accepted {
            option_index: 3,
            substitute_index: None,
        };
        assert_eq!(
            choice.validate_against(&means()),
            Err(ReframeError::OptionOutOfRange(3))
        );
    }

    #[test]
    fn substitute_index_must_be_in_range() {
        let choice = ReframeChoice::Accepted {
            option_index: 0,
            substitute_index: Some(1),
        };
        assert_eq!(
            choice.validate_against(&means()),
            Err(ReframeError::SubstituteOutOfRange(1))
        );
    }

    #[test]
    fn rejected_and_deferred_are_always_valid() {
        for choice in [
            ReframeChoice::Rejected {
                reason: RejectedReason::PreferLiteral,
            },
            ReframeChoice::Deferred,
        ] {
            assert_eq!(choice.validate_against(&means()), Ok(()));
            assert_eq!(choice.validate_against(&end()), Ok(()));
        }
    }

    #[test]
    fn receipt_keeps_the_request_and_the_choice() {
        let text = render_receipt(
            &means(),
            &ReframeChoice::Rejected {
                reason: RejectedReason::PreferLiteral,
            },
            "user",
            1_793_000_000_000,
        );
        assert!(text.contains("我要买个电钻"));
        assert!(text.contains("我就是要原方案"));
        assert!(text.contains("user"));
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;

    #[test]
    fn the_prompt_file_is_the_measured_prompt() {
        assert_eq!(prompt_sha256(), REFRAME_PROMPT_SHA256);
    }

    #[test]
    fn model_output_parses_with_and_without_a_fence() {
        let body = r#"{"classification":"means","question":"穿线还是挂东西？","options":["穿线","挂东西"],"hypotheses":["联网"],"substitutes":["路由器"],"confidence":0.8}"#;
        let plain = parse_model_output(body).unwrap();
        assert_eq!(plain.classification, "means");
        let fenced = parse_model_output(&format!("```json\n{body}\n```")).unwrap();
        assert_eq!(fenced.options.len(), 2);
    }

    #[test]
    fn a_non_json_reply_is_rejected() {
        assert!(matches!(
            parse_model_output("我觉得你应该买路由器"),
            Err(ReframeError::ModelOutputInvalid(_))
        ));
    }

    #[test]
    fn an_unknown_classification_is_rejected() {
        let parsed = parse_model_output(r#"{"classification":"maybe"}"#).unwrap();
        assert!(matches!(
            parsed.into_proposal("买电钻"),
            Err(ReframeError::ModelClassificationInvalid(_))
        ));
    }

    #[test]
    fn an_end_from_the_model_keeps_no_question_options_or_substitutes() {
        let parsed = parse_model_output(
            r#"{"classification":"end","question":"why?","options":["a","b"],"substitutes":["x"]}"#,
        )
        .unwrap();
        let proposal = parsed.into_proposal("把 typo 改掉").unwrap();
        assert_eq!(proposal.classification, RequestRole::End);
        assert!(proposal.question.is_none());
        assert!(proposal.options.is_empty());
        assert!(proposal.substitutes.is_empty());
        assert_eq!(proposal.validate(), Ok(()));
    }

    #[test]
    fn a_derived_stop_rule_is_marked_as_derived() {
        let parsed = parse_model_output(
            r#"{"classification":"means","question":"为什么？","options":["a","b"],"substitutes":["x"]}"#,
        )
        .unwrap();
        let proposal = parsed.into_proposal("买电钻").unwrap();
        assert_eq!(proposal.stop_rule_source, StopRuleSource::Derived);
        assert!(proposal.stop_rule.passes());
        assert_eq!(proposal.validate(), Ok(()));
    }

    #[test]
    fn a_means_without_substitutes_fails_the_stop_rule() {
        let parsed = parse_model_output(
            r#"{"classification":"means","question":"为什么？","options":["a","b"]}"#,
        )
        .unwrap();
        let proposal = parsed.into_proposal("买电钻").unwrap();
        assert_eq!(proposal.validate(), Err(ReframeError::StopRuleNotSatisfied));
    }

    #[test]
    fn the_user_message_carries_constraints() {
        let message = build_user_message("我要买个电钻", &["承重墙".into(), "不改结构".into()]);
        assert!(message.contains("我要买个电钻"));
        assert!(message.contains("承重墙；不改结构"));
        assert!(build_user_message("x", &[]).contains("已知约束：无"));
    }
}
