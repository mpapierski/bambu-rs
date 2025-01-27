use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct PushingPayload {
    pub sequence_id: SmolStr,

    #[serde(flatten)]
    pub command: PushingCommand,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum PushingCommand {
    #[serde(rename = "pushall")]
    PushAll,

    #[serde(rename = "start")]
    Start,
}
