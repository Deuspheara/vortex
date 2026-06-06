use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

id_type!(ProjectId);
id_type!(SessionId);
id_type!(RunId);
id_type!(EventId);
id_type!(ToolCallId);
id_type!(PatchId);
id_type!(ApprovalId);
id_type!(CheckpointId);
id_type!(ModelId);

pub type ToolName = String;
