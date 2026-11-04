//! Requirement regression: is this request a goal, or a means to one?
//!
//! The type here is the auditable half of the capability. It carries no model
//! behaviour; it only decides whether a proposal is well formed and whether the
//! stop rule was actually satisfied. Everything a model produces must pass
//! [`ReframeProposal::validate`] before it can appear in a journal, a receipt or
//! a transcript.

use serde::{Deserialize, Serialize};

use crate::safety::contains_disallowed_control;

pub const REFRAME_SCHEMA_VERSION: u32 = 1;
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
}
