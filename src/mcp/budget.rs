use crate::config::ContextConfig;

pub struct ContextBudget {
    config: ContextConfig,
    used_bytes: u64,
    tool_result_count: usize,
}

impl ContextBudget {
    pub fn new(config: ContextConfig) -> Self {
        Self {
            config,
            used_bytes: 0,
            tool_result_count: 0,
        }
    }

    pub fn can_allocate(&self, size_bytes: u64) -> bool {
        self.used_bytes + size_bytes <= self.config.max_session_context_bytes
    }

    pub fn allocate(&mut self, size_bytes: u64) -> bool {
        if self.can_allocate(size_bytes) {
            self.used_bytes += size_bytes;
            self.tool_result_count += 1;
            true
        } else {
            false
        }
    }

    pub fn remaining_bytes(&self) -> u64 {
        self.config.max_session_context_bytes.saturating_sub(self.used_bytes)
    }

    pub fn usage(&self) -> u64 {
        self.used_bytes
    }

    pub fn tool_count(&self) -> usize {
        self.tool_result_count
    }

    pub fn reset(&mut self) {
        self.used_bytes = 0;
        self.tool_result_count = 0;
    }

    pub fn limit(&self) -> u64 {
        self.config.max_session_context_bytes
    }
}
