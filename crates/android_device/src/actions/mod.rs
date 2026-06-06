pub mod target_resolver;

pub use target_resolver::{
    AndroidTarget, ResolvedTarget, TargetConfidence, TextMatchMode, resolve_target,
};
