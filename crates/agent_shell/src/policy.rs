#[derive(Clone, Debug)]
pub struct ShellPolicy {
    pub allow_writes: bool,
    pub allow_network: bool,
    pub allow_recursive_delete: bool,
    pub max_file_read_bytes: usize,
    pub max_output_bytes: usize,
    pub max_commands_per_exec: usize,
    pub max_pipeline_depth: usize,
}

impl Default for ShellPolicy {
    fn default() -> Self {
        Self {
            allow_writes: false,
            allow_network: false,
            allow_recursive_delete: false,
            max_file_read_bytes: 256 * 1024,
            max_output_bytes: 256 * 1024,
            max_commands_per_exec: 16,
            max_pipeline_depth: 1,
        }
    }
}
