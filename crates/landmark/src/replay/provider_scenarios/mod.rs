use crate::*;
mod consumers;
mod fabrication_gate;
mod failures;
mod grounding;
mod pr_pagination;
mod pr_scoping;
mod prestable;
mod provider_runs;
mod release_body_idempotency;
mod self_release;
mod semver_evidence;
mod synthesis_cost;

pub(crate) use consumers::*;
pub(crate) use fabrication_gate::*;
pub(crate) use failures::*;
pub(crate) use grounding::*;
pub(crate) use pr_pagination::*;
pub(crate) use pr_scoping::*;
pub(crate) use prestable::*;
pub(crate) use provider_runs::*;
pub(crate) use release_body_idempotency::*;
pub(crate) use self_release::*;
pub(crate) use semver_evidence::*;
pub(crate) use synthesis_cost::*;

#[derive(Default)]
pub(crate) struct FakeState {
    pub(crate) llm_status: u16,
    pub(crate) llm_notes: String,
    pub(crate) llm_responses: VecDeque<(u16, String)>,
    pub(crate) update_status: u16,
    pub(crate) releases: BTreeMap<String, Value>,
    pub(crate) requests: Vec<Value>,
    pub(crate) pull_requests: Vec<Value>,
}
