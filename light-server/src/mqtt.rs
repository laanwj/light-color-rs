use log::{debug, info, warn};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};

use light_protocol::{ModeType, State};

use crate::configuration;
use crate::{LightCommand, update_light, update_state};

// HA MQTT JSON schema types

#[derive(Serialize)]
struct Discovery {
    name: String,
    unique_id: String,
    command_topic: String,
    state_topic: String,
    schema: &'static str,
    brightness: bool,
    brightness_scale: u16,
    color_mode: bool,
    supported_color_modes: Vec<&'static str>,
    min_mireds: u16,
    max_mireds: u16,
}

#[derive(Deserialize, Debug)]
struct HaCommand {
    state: Option<String>,
    brightness: Option<u16>,
    color_temp: Option<u16>,
    color: Option<HaColor>,
}

#[derive(Deserialize, Debug)]
struct HaColor {
    h: Option<f32>,
    s: Option<f32>,
}

#[derive(Serialize)]
struct HaState {
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    brightness: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color_mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color_temp: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<HaColorOut>,
}

#[derive(Serialize)]
struct HaColorOut {
    h: f32,
    s: f32,
}

fn kelvin_to_mireds(kelvin: u16) -> u16 {
    if kelvin == 0 {
        return 370;
    }
    (1_000_000u32 / kelvin as u32) as u16
}

fn mireds_to_kelvin(mireds: u16) -> u16 {
    if mireds == 0 {
        return 7500;
    }
    (1_000_000u32 / mireds as u32) as u16
}

fn state_to_ha(state: &State) -> HaState {
    let on = state.on.unwrap_or(true);
    if !on {
        return HaState {
            state: "OFF",
            brightness: state.dim,
            color_mode: None,
            color_temp: None,
            color: None,
        };
    }
    match state.mode {
        Some(ModeType::CCT) => HaState {
            state: "ON",
            brightness: state.dim,
            color_mode: Some("color_temp"),
            color_temp: Some(kelvin_to_mireds(state.ct.unwrap_or(4000))),
            color: None,
        },
        Some(ModeType::HSI) => HaState {
            state: "ON",
            brightness: state.dim,
            color_mode: Some("hs"),
            color_temp: None,
            color: Some(HaColorOut {
                h: state.hue.unwrap_or(0) as f32,
                s: state.sat.unwrap_or(100) as f32,
            }),
        },
        None => HaState {
            state: "OFF",
            brightness: None,
            color_mode: None,
            color_temp: None,
            color: None,
        },
    }
}

fn ha_command_to_state(cmd: &HaCommand, current: &State) -> State {
    let mut state = State {
        on: None,
        mode: None,
        dim: None,
        ct: None,
        gm: None,
        hue: None,
        sat: None,
    };

    // Handle ON/OFF
    if let Some(ref s) = cmd.state {
        if s == "OFF" {
            state.on = Some(false);
            return state;
        }
        state.on = Some(true);
        // If no mode set yet, default to CCT
        if cmd.color_temp.is_none() && cmd.color.is_none() && current.mode.is_none() {
            state.mode = Some(ModeType::CCT);
            state.ct = Some(4000);
            state.gm = Some(0);
        }
        // If no brightness set yet, default to 100
        if cmd.brightness.is_none() && current.dim.is_none() {
            state.dim = Some(100);
        }
    }

    if let Some(brightness) = cmd.brightness {
        state.dim = Some(brightness.min(100));
    }

    if let Some(mireds) = cmd.color_temp {
        let kelvin = mireds_to_kelvin(mireds).clamp(2700, 7500);
        state.mode = Some(ModeType::CCT);
        state.ct = Some(kelvin);
        state.gm = Some(current.gm.unwrap_or(0));
    }

    if let Some(ref color) = cmd.color {
        state.mode = Some(ModeType::HSI);
        if let Some(h) = color.h {
            state.hue = Some((h as u16).min(360));
        }
        if let Some(s) = color.s {
            state.sat = Some((s as u16).min(100));
        }
    }

    state
}

async fn publish_state(client: &AsyncClient, prefix: &str, idx: usize, state: &State) {
    let topic = format!("{}/light/{}/state", prefix, idx);
    let ha_state = state_to_ha(state);
    match serde_json::to_string(&ha_state) {
        Ok(payload) => {
            if let Err(e) = client
                .publish(&topic, QoS::AtLeastOnce, true, payload)
                .await
            {
                warn!("Failed to publish state to {}: {}", topic, e);
            }
        }
        Err(e) => warn!("Failed to serialize state: {}", e),
    }
}

pub async fn mqtt_task(
    mqtt_config: &configuration::Mqtt,
    light_config: &[configuration::Light],
    light_states: Arc<Mutex<Vec<State>>>,
    light_tx: mpsc::Sender<(u16, LightCommand)>,
    mut state_rx: broadcast::Receiver<(usize, State)>,
) {
    let prefix = &mqtt_config.topic_prefix;
    let client_id = format!("{}-server", prefix);
    let (host, port) = mqtt_config.broker_host_port();

    let mut options = MqttOptions::new(&client_id, host, port);
    options.set_keep_alive(std::time::Duration::from_secs(30));

    if let (Some(user), Some(pass)) = (&mqtt_config.username, &mqtt_config.password) {
        options.set_credentials(user, pass);
    }

    let (client, mut eventloop) = AsyncClient::new(options, 64);

    info!("MQTT: connecting to {}", mqtt_config.broker_addr);

    // Publish discovery and subscribe to command topics for each light
    for (idx, light) in light_config.iter().enumerate() {
        let name = light
            .name
            .clone()
            .unwrap_or_else(|| format!("Nanlite Light {}", idx + 1));
        let unique_id = format!("{}_light_{}", prefix, idx);

        let discovery = Discovery {
            name,
            unique_id: unique_id.clone(),
            command_topic: format!("{}/light/{}/set", prefix, idx),
            state_topic: format!("{}/light/{}/state", prefix, idx),
            schema: "json",
            brightness: true,
            brightness_scale: 100,
            color_mode: true,
            supported_color_modes: vec!["color_temp", "hs"],
            min_mireds: 133, // 7500K
            max_mireds: 370, // 2700K
        };

        let discovery_topic = format!("homeassistant/light/{}/config", unique_id);
        let payload = serde_json::to_string(&discovery).unwrap();

        if let Err(e) = client
            .publish(&discovery_topic, QoS::AtLeastOnce, true, payload)
            .await
        {
            warn!("Failed to publish discovery for light {}: {}", idx, e);
        }

        let cmd_topic = format!("{}/light/{}/set", prefix, idx);
        if let Err(e) = client.subscribe(&cmd_topic, QoS::AtLeastOnce).await {
            warn!("Failed to subscribe to {}: {}", cmd_topic, e);
        }

        // Publish initial state
        let state_clone = light_states.lock().unwrap()[idx].clone();
        publish_state(&client, prefix, idx, &state_clone).await;
    }

    info!("MQTT: discovery published, listening for commands");

    loop {
        tokio::select! {
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        debug!("MQTT: received on {}: {:?}", publish.topic, std::str::from_utf8(&publish.payload));

                        // Parse topic: prefix/light/{idx}/set
                        let parts: Vec<&str> = publish.topic.split('/').collect();
                        if parts.len() != 4 || parts[1] != "light" || parts[3] != "set" {
                            continue;
                        }
                        let idx: usize = match parts[2].parse() {
                            Ok(i) => i,
                            Err(_) => continue,
                        };
                        if idx >= light_config.len() {
                            warn!("MQTT: light index {} out of range", idx);
                            continue;
                        }

                        let ha_cmd: HaCommand = match serde_json::from_slice(&publish.payload) {
                            Ok(cmd) => cmd,
                            Err(e) => {
                                warn!("MQTT: failed to parse command: {}", e);
                                continue;
                            }
                        };

                        debug!("MQTT: parsed command for light {}: {:?}", idx, ha_cmd);

                        let (update_command, state_clone) = {
                            let mut states = light_states.lock().unwrap();
                            let new_state = ha_command_to_state(&ha_cmd, &states[idx]);
                            update_state(&mut states[idx], &new_state);
                            let cmd = update_light(&states[idx]);
                            let state_clone = states[idx].clone();
                            (cmd, state_clone)
                        };
                        publish_state(&client, prefix, idx, &state_clone).await;

                        if let Some(light_cmd) = update_command {
                            if let Err(e) = light_tx
                                .send((light_config[idx].address, light_cmd))
                                .await
                            {
                                warn!("MQTT: failed to send light command: {}", e);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("MQTT: connection error: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
            state_update = state_rx.recv() => {
                match state_update {
                    Ok((idx, state)) => {
                        publish_state(&client, prefix, idx, &state).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("MQTT: skipped {} state updates", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("MQTT: state broadcast channel closed");
                        break;
                    }
                }
            }
        }
    }
}
