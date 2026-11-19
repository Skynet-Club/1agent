//! Deterministic IRIs for ontology entities.
//!
//! Segment rules follow the P0 ontology contract: only
//! `A-Z a-z 0-9 - . _ ~` stay literal; every other UTF-8 byte is
//! percent-encoded with uppercase hex. Process ids, paths and titles are
//! attributes, never identity inputs.

pub fn encode_segment(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        let character = *byte as char;
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '~') {
            output.push(character);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    if output.is_empty() {
        // Keep a blank identity from producing a dangling IRI separator.
        "%00".into()
    } else {
        output
    }
}

pub fn session_id(id: &str) -> String {
    format!("urn:1agent:session:{}", encode_segment(id))
}

pub fn run_id(id: &str) -> String {
    format!("urn:1agent:run:{}", encode_segment(id))
}

pub fn task_id(run: &str) -> String {
    format!("urn:1agent:task:{}:main", encode_segment(run))
}

pub fn tool_call_id(run: &str, call: &str) -> String {
    format!(
        "urn:1agent:tool-call:{}:{}",
        encode_segment(run),
        encode_segment(call)
    )
}

pub fn observation_id(run: &str, call: &str) -> String {
    format!(
        "urn:1agent:observation:{}:{}",
        encode_segment(run),
        encode_segment(call)
    )
}

pub fn answer_id(run: &str) -> String {
    format!("urn:1agent:answer:{}", encode_segment(run))
}

pub fn decision_id(run: &str) -> String {
    format!("urn:1agent:decision:{}:main", encode_segment(run))
}

pub fn outcome_id(run: &str) -> String {
    format!("urn:1agent:outcome:{}", encode_segment(run))
}

pub fn option_id(run: &str, index: usize) -> String {
    format!("urn:1agent:option:{}:{}", encode_segment(run), index)
}

pub fn criterion_id(run: &str, key: &str) -> String {
    format!(
        "urn:1agent:criterion:{}:{}",
        encode_segment(run),
        encode_segment(key)
    )
}

pub fn claim_id(run: &str, claim: &str) -> String {
    format!(
        "urn:1agent:claim:{}:{}",
        encode_segment(run),
        encode_segment(claim)
    )
}

pub fn evidence_id(run: &str, seq: u64, index: usize) -> String {
    format!(
        "urn:1agent:evidence:{}:{}:{}",
        encode_segment(run),
        seq,
        index
    )
}

pub fn artifact_id(sha256: &str) -> String {
    format!("urn:1agent:artifact:sha256:{}", encode_segment(sha256))
}

pub fn goal_id(id: &str) -> String {
    format!("urn:1agent:goal:{}", encode_segment(id))
}

pub fn commitment_id(id: &str) -> String {
    format!("urn:1agent:commitment:{}", encode_segment(id))
}

pub fn squad_member_id(run: &str, call: &str) -> String {
    format!(
        "urn:1agent:squad-member:{}:{}",
        encode_segment(run),
        encode_segment(call)
    )
}

pub fn action_invocation_id(run: &str, call: &str) -> String {
    format!(
        "urn:1agent:action:{}:{}",
        encode_segment(run),
        encode_segment(call)
    )
}

pub fn policy_decision_id(run: &str, call: &str) -> String {
    format!(
        "urn:1agent:policy-decision:{}:{}",
        encode_segment(run),
        encode_segment(call)
    )
}

pub fn approval_id(run: &str, call: &str) -> String {
    format!(
        "urn:1agent:approval:{}:{}",
        encode_segment(run),
        encode_segment(call)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_segment_preserves_unreserved_characters() {
        assert_eq!(encode_segment("AZaz09-._~"), "AZaz09-._~");
    }

    #[test]
    fn encode_segment_percent_encodes_reserved_and_unicode() {
        assert_eq!(encode_segment("a b/c:d?e#f%g"), "a%20b%2Fc%3Ad%3Fe%23f%25g");
        assert_eq!(encode_segment("证据"), "%E8%AF%81%E6%8D%AE");
    }

    #[test]
    fn encode_segment_keeps_blank_identities_non_empty() {
        assert_eq!(encode_segment(""), "%00");
        assert_eq!(encode_segment(""), encode_segment(""));
    }

    #[test]
    fn core_ids_follow_the_contract_shape() {
        assert_eq!(session_id("ses_1"), "urn:1agent:session:ses_1");
        assert_eq!(run_id("run-1"), "urn:1agent:run:run-1");
        assert_eq!(task_id("run-1"), "urn:1agent:task:run-1:main");
        assert_eq!(
            tool_call_id("run-1", "call-1"),
            "urn:1agent:tool-call:run-1:call-1"
        );
        assert_eq!(
            observation_id("run-1", "call-1"),
            "urn:1agent:observation:run-1:call-1"
        );
        assert_eq!(answer_id("run-1"), "urn:1agent:answer:run-1");
        assert_eq!(decision_id("run-1"), "urn:1agent:decision:run-1:main");
        assert_eq!(outcome_id("run-1"), "urn:1agent:outcome:run-1");
        assert_eq!(option_id("run-1", 0), "urn:1agent:option:run-1:0");
        assert_eq!(
            criterion_id("run-1", "verifiable"),
            "urn:1agent:criterion:run-1:verifiable"
        );
        assert_eq!(claim_id("run-1", "c1"), "urn:1agent:claim:run-1:c1");
        assert_eq!(
            evidence_id("run-1", 12, 0),
            "urn:1agent:evidence:run-1:12:0"
        );
        assert_eq!(
            artifact_id("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            "urn:1agent:artifact:sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            action_invocation_id("run-1", "call-1"),
            "urn:1agent:action:run-1:call-1"
        );
        assert_eq!(
            policy_decision_id("run-1", "call-1"),
            "urn:1agent:policy-decision:run-1:call-1"
        );
        assert_eq!(
            approval_id("run-1", "call-1"),
            "urn:1agent:approval:run-1:call-1"
        );
        assert_eq!(goal_id("goal-1"), "urn:1agent:goal:goal-1");
        assert_eq!(
            commitment_id("commitment-1"),
            "urn:1agent:commitment:commitment-1"
        );
        assert_eq!(
            squad_member_id("run-1", "call-1"),
            "urn:1agent:squad-member:run-1:call-1"
        );
    }

    #[test]
    fn segments_are_encoded_inside_composite_ids() {
        assert_eq!(task_id("run/1"), "urn:1agent:task:run%2F1:main");
        assert_eq!(
            tool_call_id("run:1", "call/1"),
            "urn:1agent:tool-call:run%3A1:call%2F1"
        );
        assert_eq!(
            observation_id("run:1", "call/1"),
            "urn:1agent:observation:run%3A1:call%2F1"
        );
        assert_eq!(answer_id("run:1"), "urn:1agent:answer:run%3A1");
        assert_eq!(
            claim_id("run:1", "claim 1"),
            "urn:1agent:claim:run%3A1:claim%201"
        );
        assert_eq!(
            evidence_id("run:1", 12, 0),
            "urn:1agent:evidence:run%3A1:12:0"
        );
        assert_eq!(goal_id("goal 1"), "urn:1agent:goal:goal%201");
        assert_eq!(commitment_id("c:1"), "urn:1agent:commitment:c%3A1");
    }

    #[test]
    fn ids_are_stable_and_do_not_merge_distinct_inputs() {
        assert_eq!(session_id("ses_1"), session_id("ses_1"));
        assert_eq!(
            tool_call_id("run:1", "call-1"),
            tool_call_id("run:1", "call-1")
        );
        assert_ne!(
            tool_call_id("run:1", "call-1"),
            tool_call_id("run", "1:call-1"),
            "separator characters must not shift between segments"
        );
        assert_ne!(session_id("run-1"), run_id("run-1"));
        assert_ne!(answer_id("run-1"), run_id("run-1"));
        assert_ne!(goal_id("g1"), commitment_id("g1"));
    }

    #[test]
    fn path_and_title_like_inputs_remain_opaque_segments() {
        assert_eq!(
            session_id("/home/user/session.json"),
            "urn:1agent:session:%2Fhome%2Fuser%2Fsession.json"
        );
        assert_eq!(
            session_id("Title: A/B"),
            "urn:1agent:session:Title%3A%20A%2FB"
        );
        assert_eq!(session_id("pid:4242"), "urn:1agent:session:pid%3A4242");
    }
}
