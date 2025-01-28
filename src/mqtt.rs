pub(crate) mod command;
pub mod message;

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use command::{
    info::{InfoCommand, InfoPayload},
    system::{LedCtrl, LedMode, LedNode, SystemCommand, SystemPayload},
    Command,
};
use rumqttc::{
    tokio_rustls::rustls::ClientConfig, AsyncClient, ClientError, Event, MqttOptions, Packet, QoS,
    TlsConfiguration, Transport,
};
use smol_str::{format_smolstr, SmolStr};
use thiserror::Error;
use tokio::{
    sync::{oneshot, Mutex},
    task::JoinHandle,
    time::Duration,
};

use crate::tls::NoVerifier;
use message::{info::Info, system::System, Message};

#[derive(Debug, Error)]
pub enum MqttError {
    #[error("MQTT error: {0}")]
    ClientError(#[from] ClientError),
    #[error("Failed to serialize command: {0}")]
    SerdeError(#[from] serde_json::Error),
}

const DEFAULT_MQTT_ID: &str = "bblp_client";
const DEFAULT_MQTT_PORT: u16 = 8883;
const DEFAULT_MQTT_USERNAME: &str = "bblp";

/// Main watch client.
pub struct MqttClient {
    hostname: String,
    access_code: String,
    serial: String,
    /// We'll store a reference to the asynchronous MQTT client and its event loop.
    /// The event loop is run on a background task.
    client: Option<Arc<AsyncClient>>,
    /// A signal for stopping the event loop
    stop_flag: Arc<Mutex<bool>>,
    /// A map of inflight requests (keyed by sequence_id).
    inflight_commands: Arc<Mutex<HashMap<SmolStr, oneshot::Sender<Message>>>>,
    /// Current sequence id.
    sequence_id: Mutex<u64>,
}

impl MqttClient {
    /// Create a new WatchClient.
    pub fn new(hostname: &str, access_code: &str, serial: &str) -> Self {
        Self {
            hostname: hostname.to_string(),
            access_code: access_code.to_string(),
            serial: serial.to_string(),
            client: None,
            stop_flag: Arc::new(Mutex::new(false)),
            inflight_commands: Default::default(),
            sequence_id: Mutex::new(0),
        }
    }

    /// Start the MQTT client.
    ///
    /// This spawns a background task that processes MQTT events.
    pub async fn start(&mut self) -> Result<JoinHandle<()>> {
        // 1) Build MqttOptions
        let mut mqttoptions =
            MqttOptions::new(DEFAULT_MQTT_ID, self.hostname.clone(), DEFAULT_MQTT_PORT);

        // Set username & password
        mqttoptions.set_credentials(DEFAULT_MQTT_USERNAME, &self.access_code);
        mqttoptions.set_keep_alive(Duration::from_secs(60));

        // 2) Configure TLS ignoring certificate validation
        // rumqttc uses rustls internally. We'll supply a dangerous configuration.
        let config: ClientConfig = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();

        mqttoptions.set_transport(Transport::Tls(TlsConfiguration::Rustls(Arc::new(config))));

        // 3) Create the AsyncClient and EventLoop
        let (client, mut event_loop) = AsyncClient::new(mqttoptions, 10);
        let client = Arc::new(client);
        self.client = Some(Arc::clone(&client));

        // 4) Mark `stop_flag = false`
        {
            let mut stop = self.stop_flag.lock().await;
            *stop = false;
        }

        // 5) Spawn a background task that processes the event loop
        let stop_flag = self.stop_flag.clone();

        let serial = self.serial.clone();

        let (connected_tx, connected_rx) = oneshot::channel();

        let handle = tokio::spawn({
            let inflight_commands = Arc::clone(&self.inflight_commands);

            async move {
                let mut connected_tx = Some(connected_tx);

                // We subscribe once we see a successful connection (Event::Connected).
                // Then we listen for packets in a loop.
                loop {
                    tokio::select! {
                        evt = event_loop.poll() => {
                            match evt {
                                Ok(Event::Incoming(incoming)) => {
                                    match incoming {
                                        Packet::ConnAck(_ack) => {
                                            // "Connected" event
                                            // Subscribe to `device/{serial}/report`
                                            let topic = format!("device/{}/report", serial);
                                            if let Err(e) = client.subscribe(topic.clone(), QoS::AtMostOnce).await {
                                                eprintln!("Failed to subscribe to {}: {:?}", topic, e);
                                                break;
                                            }

                                            // Notify the main task that we are connected
                                            if let Some(tx) = connected_tx.take() {
                                                tx.send(Ok(())).unwrap();
                                            }
                                        }
                                        Packet::Publish(publish) => {
                                            // Check topic if it matches the one we subscribed
                                            // (or handle multiple topics if needed)
                                            let topic = publish.topic.clone();
                                            let payload = publish.payload;

                                            match serde_json::from_slice::<Message>(&payload) {
                                                Ok(Message::Print(print)) if print.command == "push_status" => {
                                                    // Pushed message for which there is no inflight command.
                                                    println!("Received pushed message from {topic}: {:?}", print);
                                                }
                                                Ok(msg) => {
                                                    // Handle the message here.
                                                    let mut inflight_commands = Arc::clone(&inflight_commands).lock_owned().await;

                                                    match inflight_commands.remove(msg.sequence_id()) {
                                                        Some(inflight_command) => {
                                                            println!("Received message from {topic}: {:?}", msg);

                                                            // Send the response back to the command sender.
                                                            inflight_command.send(msg).unwrap();
                                                        }
                                                        None => {
                                                            eprintln!("Received message with unknown sequence_id: {msg:?}");
                                                        }
                                                    }

                                                }
                                                Err(err) => {
                                                    eprintln!("Failed to parse MQTT payload from {topic}: {:?} (payload: {})", err, String::from_utf8_lossy(&payload));
                                                }
                                            }
                                        }
                                        _ => {} // Handle other packets if needed
                                    }
                                }
                                Ok(Event::Outgoing(_)) => {
                                    // Outgoing events, usually not needed to handle
                                }
                                Err(e) => {
                                    eprintln!("MQTT error: {:?}", e);

                                    if let Some(tx) = connected_tx.take() {
                                        tx.send(Err(e)).unwrap();
                                    }
                                    break;
                                }
                            }
                        }
                        // If `stop_flag` is set to true, break out
                        _ = async {
                            let mut interval = tokio::time::interval(Duration::from_millis(500));
                            loop {
                                interval.tick().await;
                                if *stop_flag.lock().await {
                                    break;
                                }
                            }
                        } => {
                            // We are asked to stop
                            break;
                        }
                    }
                }

                // We are done: attempt a graceful shutdown
                let _ = client.disconnect().await;
            }
        });

        // Wait for connection to be established
        match connected_rx.await.unwrap() {
            Ok(()) => {}
            Err(e) => return Err(e.into()),
        }

        Ok(handle)
    }

    /// Stop the MQTT loop and disconnect.
    pub async fn stop(&mut self) -> Result<()> {
        // Signal the background task to end
        {
            let mut stop = self.stop_flag.lock().await;
            *stop = true;
        }

        Ok(())
    }

    /// Send a command to the printer.
    pub(crate) async fn send_raw_command_and_wait(
        &mut self,
        command: Command,
    ) -> Result<Message, MqttError> {
        // Serialize the command
        let payload = serde_json::to_vec(&command)?;

        // Publish the command
        let topic = format!("device/{}/request", self.serial);
        let qos = QoS::AtMostOnce;

        let (tx, rx) = oneshot::channel();

        // Clone the sequence_id so we can store it in the inflight_commands map. This way we can match the response to the command.
        let sequence_id = command.sequence_id().clone();

        let client = Arc::clone(self.client.as_ref().unwrap());

        // Store the command in the inflight_commands map
        {
            let mut inflight_commands = self.inflight_commands.lock().await;
            inflight_commands.insert(sequence_id, tx);
        }

        eprintln!(
            "Publishing command to {}: {}",
            topic,
            String::from_utf8_lossy(&payload)
        );

        // Publish the command to the MQTT broker and wait for the response to arrive in the oneshot channel (rx) we created.
        client.publish(topic, qos, false, payload).await?;

        // Wait for the response to arrive in the oneshot channel.
        let response = rx.await.unwrap();
        Ok(response)
    }

    async fn send_command_and_wait<T>(&mut self, command: Command) -> Result<T, MqttError>
    where
        T: TryFrom<Message>,
        <T as TryFrom<Message>>::Error: std::fmt::Debug,
    {
        let message = self.send_raw_command_and_wait(command).await?;
        Ok(T::try_from(message).unwrap())
    }

    /// Get the version of the printer.
    pub async fn get_version(&mut self) -> Result<Info, MqttError> {
        let command = Command::Info {
            info: InfoPayload {
                sequence_id: self.next_sequence_id().await,
                command: InfoCommand::GetVersion,
            },
        };
        let result = self.send_command_and_wait(command).await?;
        Ok(result)
    }

    /// Set the lights on or off on the printer.
    pub async fn set_led(&mut self, on: bool) -> Result<System, MqttError> {
        let led_mode = if on { LedMode::On } else { LedMode::Off };
        let command = Command::System {
            system: SystemPayload {
                sequence_id: self.next_sequence_id().await,
                command: SystemCommand::LedCtrl(LedCtrl {
                    led_node: LedNode::ChamberLight,
                    led_mode,
                    led_on_time: 500,
                    led_off_time: 500,
                    loop_times: 0,
                    interval_time: 0,
                }),
            },
        };
        self.send_command_and_wait(command).await
    }

    /// Get the next sequence id.
    pub(crate) async fn next_sequence_id(&self) -> SmolStr {
        let mut sequence_id = self.sequence_id.lock().await;
        let result = format_smolstr!("{}", *sequence_id);
        *sequence_id += 1;
        result
    }
}
