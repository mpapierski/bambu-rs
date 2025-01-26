use crate::tls::NoVerifier;
use anyhow::Result;
use rumqttc::tokio_rustls::rustls::ClientConfig;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::Duration;

/// Represents the printer status.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Print {
    pub bed_temper: Option<f64>,
    pub nozzle_temper: Option<f64>,
    pub command: String,
    pub msg: u64,
    pub sequence_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Message {
    #[serde(rename = "print")]
    Print(Print),
}

/// Main watch client.
pub struct MqttClient {
    hostname: String,
    access_code: String,
    serial: String,
    /// We'll store a reference to the asynchronous MQTT client and its event loop.
    /// The event loop is run on a background task.
    client: Option<AsyncClient>,
    /// Store the latest printer status
    printer_status: Arc<Mutex<Option<Print>>>,
    /// A signal for stopping the event loop
    stop_flag: Arc<Mutex<bool>>,
}

impl MqttClient {
    /// Create a new WatchClient.
    pub fn new(hostname: &str, access_code: &str, serial: &str) -> Self {
        Self {
            hostname: hostname.to_string(),
            access_code: access_code.to_string(),
            serial: serial.to_string(),
            client: None,
            printer_status: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(Mutex::new(false)),
        }
    }

    /// Start the MQTT client.
    ///
    /// This spawns a background task that processes MQTT events.
    pub async fn start(&mut self) -> Result<JoinHandle<()>> {
        // 1) Build MqttOptions
        let mut mqttoptions = MqttOptions::new("bblp_client", self.hostname.clone(), 8883);

        // Set username & password
        mqttoptions.set_credentials("bblp", &self.access_code);
        mqttoptions.set_keep_alive(Duration::from_secs(60));

        // 2) Configure TLS ignoring certificate validation
        // rumqttc uses rustls internally. We'll supply a dangerous configuration.
        // If you have valid CA or self-signed cert, handle it properly.
        let config: ClientConfig = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();

        mqttoptions.set_transport(Transport::Tls(TlsConfiguration::Rustls(Arc::new(config))));

        // 3) Create the AsyncClient and EventLoop
        let (client, mut event_loop) = AsyncClient::new(mqttoptions, 10);
        self.client = Some(client.clone());

        // 4) Mark `stop_flag = false`
        {
            let mut stop = self.stop_flag.lock().unwrap();
            *stop = false;
        }

        // 5) Spawn a background task that processes the event loop
        let stop_flag = self.stop_flag.clone();
        let printer_status_shared = self.printer_status.clone();
        let serial = self.serial.clone();

        let (connected_tx, connected_rx) = oneshot::channel();

        let handle = tokio::spawn({
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
                                            let topic = format!("device/{}/#", serial);
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
                                                Ok(msg) => {
                                                    println!("Received message from {topic}: {:?}", msg);
                                                    match msg {
                                                        Message::Print(print) => {
                                                            // Update shared printer_status
                                                            let mut ps_lock = printer_status_shared.lock().unwrap();
                                                            *ps_lock = Some(print.clone());
                                                        }
                                                    }
                                                }
                                                Err(err) => {
                                                    eprintln!("Failed to parse MQTT payload: {:?} (payload: {})", err, String::from_utf8_lossy(&payload));
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
                                if *stop_flag.lock().unwrap() {
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
            let mut stop = self.stop_flag.lock().unwrap();
            *stop = true;
        }

        Ok(())
    }

    /// Get the last known printer status (if any).
    pub fn printer_status(&self) -> Option<Print> {
        self.printer_status.lock().unwrap().clone()
    }
}
