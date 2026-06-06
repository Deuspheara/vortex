#[derive(Clone, Debug)]
pub struct ExecutionLimits {
    pub max_output_bytes: usize,
    pub max_command_count: usize,
    pub max_file_read_bytes: usize,
    pub max_traversal_entries: usize,
    pub max_grep_matches: usize,
    pub max_recursion_depth: usize,
    pub timeout_ms: u64,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: 256 * 1024,
            max_command_count: 16,
            max_file_read_bytes: 256 * 1024,
            max_traversal_entries: 2_000,
            max_grep_matches: 200,
            max_recursion_depth: 8,
            timeout_ms: 30_000,
        }
    }
}
