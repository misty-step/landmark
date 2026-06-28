use crate::*;
mod consumers;
mod failures;
mod provider_runs;
mod self_release;
mod synthesis_cost;

pub(crate) use consumers::*;
pub(crate) use failures::*;
pub(crate) use provider_runs::*;
pub(crate) use self_release::*;
pub(crate) use synthesis_cost::*;

#[derive(Default)]
pub(crate) struct FakeState {
    pub(crate) llm_status: u16,
    pub(crate) llm_notes: String,
    pub(crate) llm_responses: VecDeque<(u16, String)>,
    pub(crate) update_status: u16,
    pub(crate) releases: BTreeMap<String, Value>,
    pub(crate) requests: Vec<Value>,
}
