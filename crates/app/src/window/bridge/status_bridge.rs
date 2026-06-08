//! Status bridge — sync agent status.

use super::super::AgentWindow;
use crate::features::shell::state::AgentStatus;

impl AgentWindow {
    pub(crate) fn sync_agent_status(&mut self, status: AgentStatus) {
        self.status.agent_status = Some(status);
        self.refresh_session_run_state();
    }

    pub(crate) fn reset_agent_status_to_idle(&mut self) {
        self.sync_agent_status(AgentStatus::Idle);
    }
}
