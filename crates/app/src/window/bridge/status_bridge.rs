//! Status bridge — sync agent status.

use super::super::AgentWindow;
use crate::features::shell::state::AgentStatus;

impl AgentWindow {
    pub(crate) fn sync_agent_status(&mut self, status: AgentStatus) {
        self.status.agent_status = Some(status);
        self.refresh_session_run_state();
    }
}
