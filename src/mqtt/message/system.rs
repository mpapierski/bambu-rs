use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::mqtt::command::system::LedCtrl;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct System {
    pub sequence_id: SmolStr,
    #[serde(flatten)]
    pub command: LedCtrl,
    pub reason: SmolStr,
    pub result: SmolStr,
}
