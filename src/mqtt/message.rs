pub mod info;
pub mod print;
pub mod system;

use serde::{Deserialize, Serialize};

use info::Info;
use print::Print;
use system::System;

/// The root of all MQTT messages.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Message {
    #[serde(rename = "print")]
    Print(Print),
    #[serde(rename = "info")]
    Info(Info),
    #[serde(rename = "system")]
    System(System),
}

impl TryFrom<Message> for Print {
    type Error = TryFromMessageError;

    fn try_from(message: Message) -> Result<Self, Self::Error> {
        match message {
            Message::Print(print) => Ok(print),
            _ => Err(TryFromMessageError(())),
        }
    }
}

impl TryFrom<Message> for Info {
    type Error = TryFromMessageError;

    fn try_from(message: Message) -> Result<Self, Self::Error> {
        match message {
            Message::Info(info) => Ok(info),
            _ => Err(TryFromMessageError(())),
        }
    }
}

impl TryFrom<Message> for System {
    type Error = TryFromMessageError;

    fn try_from(message: Message) -> Result<Self, Self::Error> {
        match message {
            Message::System(system) => Ok(system),
            _ => Err(TryFromMessageError(())),
        }
    }
}

#[derive(Debug)]
pub struct TryFromMessageError(());

impl Message {
    pub(crate) fn sequence_id(&self) -> &str {
        match self {
            Message::Print(print) => &print.sequence_id,
            Message::Info(info) => &info.sequence_id,
            Message::System(system) => &system.sequence_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::mqtt::{
        command::system::{LedCtrl, LedMode, LedNode},
        message::system::System,
    };

    use super::Message;

    const SERIAL_NUMBER_1: &str = "111111111111111";
    const SERIAL_NUMBER_2: &str = "222222222222222";
    const SERIAL_NUMBER_3: &str = "333333333333333";
    const SERIAL_NUMBER_4: &str = "444444444444444";

    #[test]
    fn decode_led_ctrl_response() {
        let response = json!({"system":{"sequence_id":"1","command":"ledctrl","led_node":"chamber_light","led_mode":"off","led_on_time":500,"led_off_time":500,"loop_times":0,"interval_time":0,"reason":"success","result":"success"}});
        let actual = serde_json::from_value::<Message>(response).unwrap();
        assert_eq!(
            actual,
            Message::System(System {
                sequence_id: "1".into(),
                command: LedCtrl {
                    led_node: LedNode::ChamberLight,
                    led_mode: LedMode::Off,
                    led_on_time: 500,
                    led_off_time: 500,
                    loop_times: 0,
                    interval_time: 0,
                },
                reason: "success".into(),
                result: "success".into(),
            })
        );
    }

    #[test]
    fn test_get_version_parser() {
        let payload = json!({
            "info": {
                "command": "get_version",
                "sequence_id": "0",
                "module": [
                    {
                        "name":"ota",
                        "project_name": "N2S",
                        "sw_ver": "<sw_ver>",
                        "hw_ver": "OTA",
                        "sn": SERIAL_NUMBER_1,
                        "flag": 0
                    },
                    {
                        "name": "esp32",
                        "project_name":"N2S",
                        "sw_ver": "<sw_ver>",
                        "hw_ver": "AP05",
                        "sn": SERIAL_NUMBER_1,
                        "flag": 0
                    },
                    {
                        "name": "mc",
                        "project_name": "N2S",
                        "sw_ver": "00.00.29.76",
                        "loader_ver": "<sw_ver>",
                        "hw_ver": "MC02",
                        "sn": SERIAL_NUMBER_2,
                        "flag": 0
                    },
                    {
                        "name": "th",
                        "project_name": "N2S",
                        "sw_ver": "<sw_ver>",
                        "loader_ver": "<s2_ver3>",
                        "hw_ver": "TH01",
                        "sn": SERIAL_NUMBER_3,
                        "flag": 0
                    },
                    {
                        "name": "ams_f1/0",
                        "project_name":"",
                        "sw_ver": "<sw_ver>",
                        "loader_ver": "<ver>",
                        "ota_ver": "<ver>",
                        "hw_ver": "AMS_F102",
                        "sn": SERIAL_NUMBER_4,
                        "flag": 0
                    }
                ],
                "result": "success",
                "reason": ""
            }
        });

        let _message = serde_json::from_value::<Message>(payload).unwrap();
    }
}
