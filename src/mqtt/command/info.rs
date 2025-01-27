use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct InfoPayload {
    pub sequence_id: SmolStr,

    // Flatten the command enum so it becomes e.g.
    // {
    //   "sequence_id": "123",
    //   "command": "get_version"
    // }
    #[serde(flatten)]
    pub command: InfoCommand,
}

/// The only valid command for the "info" JSON root:
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum InfoCommand {
    #[serde(rename = "get_version")]
    GetVersion,
}
