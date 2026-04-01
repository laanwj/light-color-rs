use clap::Parser;
use log::{debug, info, trace, warn};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

use light_protocol::{Command, ModeType, Response, ResponseType, State};

mod configuration;
mod mqtt;
mod nanlite;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, required = true)]
    config_file: PathBuf,
}

/** Update state record from another state record.
 * Data items that are the unset, will stay the same.
 */
fn update_state(dest_state: &mut State, src_state: &State) {
    if let Some(on) = src_state.on {
        dest_state.on = Some(on);
    }
    if let Some(mode) = src_state.mode {
        dest_state.mode = Some(mode);
    }
    if let Some(dim) = src_state.dim {
        dest_state.dim = Some(dim);
    }
    if let Some(ct) = src_state.ct {
        dest_state.ct = Some(ct);
    }
    if let Some(gm) = src_state.gm {
        dest_state.gm = Some(gm);
    }
    if let Some(hue) = src_state.hue {
        dest_state.hue = Some(hue);
    }
    if let Some(sat) = src_state.sat {
        dest_state.sat = Some(sat);
    }
}

/** Create light update command from light state (if complete) to light thread.
 * Validate and convert values to nanlite values.
 */
fn update_light(state: &State) -> Option<LightCommand> {
    let light_on = state.on.unwrap_or(true);

    match state.mode {
        Some(ModeType::CCT) => {
            let dim = state.dim.unwrap_or(0);
            let ct = state.ct?;
            let gm = state.gm?;
            // ct is mapped from 2700K..7500K
            let ct_val: u8 = if ct < 2700 {
                0
            } else if ct > 7500 {
                100
            } else {
                (((ct as u32) - 2700) * 100 / (7500 - 2700)) as u8
            };
            // gm is mapped from -100..100 to 0..100
            let gm_val: u8 = (((gm.clamp(-100, 100) as i32) + 100) / 2) as u8;
            let dim_val: u8 = if light_on { dim.min(100) as u8 } else { 0 };

            Some(LightCommand::CCT(dim_val, ct_val, gm_val))
        }
        Some(ModeType::HSI) => {
            let dim = state.dim.unwrap_or(0);
            let hue = state.hue?;
            let sat = state.sat?;
            let hue_val: u16 = hue.min(360);
            let sat_val: u8 = sat.min(100) as u8;
            let dim_val: u8 = if light_on { dim.min(100) as u8 } else { 0 };

            Some(LightCommand::HSI(hue_val, sat_val, dim_val))
        }
        None => None,
    }
}

/** Command to lights thread. */
#[derive(Debug, Copy, Clone)]
enum LightCommand {
    CCT(u8, u8, u8),
    HSI(u16, u8, u8),
}

/** Task that receives light commands, and dispatches them to the radio.
 */
async fn lights_task(
    config: &configuration::Hardware,
    mut rx: mpsc::Receiver<(u16, LightCommand)>,
) {
    info!("Light thread running");
    let mut rf24 = nanlite::rf24_init(config.device.clone(), config.nrf24_ce_gpio).unwrap();
    while let Some((idx, cmd)) = rx.recv().await {
        debug!("GOT = {:?}", (idx, cmd));
        match cmd {
            LightCommand::CCT(intensity, cct, gm) => {
                nanlite::set_intensity_cct_gm(&mut rf24, idx, intensity, cct, gm).unwrap();
            }
            LightCommand::HSI(hue, sat, intensity) => {
                nanlite::set_hue_sat_intensity(&mut rf24, idx, hue, sat, intensity).unwrap();
            }
        }
    }
}

/** Task that handles an incoming connection.
 */
async fn connection_task(
    light_config: &Vec<configuration::Light>,
    light_states: Arc<Mutex<Vec<State>>>,
    tx: mpsc::Sender<(u16, LightCommand)>,
    state_broadcast: broadcast::Sender<(usize, State)>,
    mut stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
) {
    info!("Thread {} starting", peer.to_string());
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    // Print initial state to new connection
    let response: Response = Response {
        response: ResponseType::State,
        error: None,
        state: Some(light_states.lock().unwrap().clone()),
    };
    let response_string = serde_json::to_string(&response).unwrap() + "\n";
    writer.write_all(response_string.as_bytes()).await.unwrap();

    let mut buf = vec![];
    loop {
        match buf_reader.read_until(b'\n', &mut buf).await {
            Ok(n) => {
                if n == 0 {
                    debug!("EOF received");
                    break;
                }
                let buf_string = String::from_utf8_lossy(&buf);
                trace!("Received line: {:?}", buf_string);
                let command: Command = serde_json::from_str(&buf_string).unwrap();
                debug!("Received message: {:?}", command);
                let update_command = {
                    let mut light_states_mut = light_states.lock().unwrap();
                    update_state(&mut light_states_mut[command.idx as usize], &command.state);
                    let cmd = update_light(&light_states_mut[command.idx as usize]);
                    let _ = state_broadcast.send((
                        command.idx as usize,
                        light_states_mut[command.idx as usize].clone(),
                    ));
                    cmd
                };

                // Send command to light thread.
                debug!("Out: {:?}", update_command);
                if let Some(light_cmd) = update_command {
                    tx.send((light_config[command.idx as usize].address, light_cmd))
                        .await
                        .unwrap();
                }

                // TODO: error handling for invalid input
                let response: Response = Response {
                    response: ResponseType::OK,
                    error: None,
                    state: None,
                };

                // Write response.
                let response_string = serde_json::to_string(&response).unwrap() + "\n";
                writer.write_all(response_string.as_bytes()).await.unwrap();

                buf.clear();
            }
            Err(e) => {
                warn!("Error receiving message: {}", e);
                break;
            }
        }
    }

    info!("Thread {} finishing", peer.to_string());
}

fn read_config(config_file: &Path) -> Result<configuration::Configuration, Box<dyn Error>> {
    let config_data = fs::read_to_string(config_file)?;
    Ok(serde_json::from_str(&config_data)?)
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = Cli::parse();

    let config = read_config(&cli.config_file);
    if let Err(err) = config {
        eprintln!("Error loading configuration: {}", err);
        return;
    }
    let config = config.unwrap();

    let addr = config.network.bind_addr.clone();
    let socket = TcpListener::bind(&addr).await.unwrap();

    info!("Listening on {}", addr);

    // Initial light states (unknown).
    let num_lights = config.lights.len();
    let initial_states: Vec<State> = vec![
        State {
            on: None,
            mode: None,
            dim: None,
            ct: None,
            gm: None,
            hue: None,
            sat: None
        };
        num_lights
    ];
    let light_states = Arc::new(Mutex::new(initial_states));

    // Make channel for communicating with lights thread.
    let (tx, rx) = mpsc::channel::<(u16, LightCommand)>(32);

    // Broadcast channel for state updates (notifies MQTT task of TCP-originated changes).
    let (state_broadcast, _) = broadcast::channel::<(usize, State)>(64);

    // Spawn lights thread.
    let hardware_config = config.hardware.clone();
    tokio::spawn(async move { lights_task(&hardware_config, rx).await });

    // Spawn MQTT task if configured.
    if let Some(ref mqtt_config) = config.mqtt {
        let mqtt_config = mqtt_config.clone();
        let light_config = config.lights.clone();
        let light_states = light_states.clone();
        let light_tx = tx.clone();
        let state_rx = state_broadcast.subscribe();
        tokio::spawn(async move {
            mqtt::mqtt_task(
                &mqtt_config,
                &light_config,
                light_states,
                light_tx,
                state_rx,
            )
            .await;
        });
    }

    while let Ok((stream, peer)) = socket.accept().await {
        let light_states = light_states.clone();
        let tx = tx.clone();
        let state_broadcast = state_broadcast.clone();
        let light_config = config.lights.clone();
        info!("Incoming connection from: {}", peer.to_string());
        tokio::spawn(async move {
            connection_task(
                &light_config,
                light_states,
                tx,
                state_broadcast,
                stream,
                peer,
            )
            .await;
        });
    }
}
