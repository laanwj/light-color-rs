use std::error::Error;
use std::fs;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use light_protocol::{Command, ModeType, Response, ResponseType, State};

mod configuration;
mod nanlite;

/** Update state record from another state record.
 * Data items that are the unset, will stay the same.
 */
fn update_state(dest_state: &mut State, src_state: &State) {
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
    match state.mode {
        Some(ModeType::CCT) => {
            if let (Some(dim), Some(ct), Some(gm)) = (state.dim, state.ct, state.gm) {
                // ct is mapped from 2700K..7500K
                let ct_val: u8 = if ct < 2700 {
                    0
                } else if ct > 7500 {
                    100
                } else {
                    (((ct as u32) - 2700) * 100 / (7500 - 2700)) as u8
                };
                // gm is mapped from -100..100 to 0..100
                let gm_val: u8 = if gm < -100 {
                    0
                } else if gm > 100 {
                    100
                } else {
                    (((gm as i32) + 100) / 2) as u8
                };
                // Check range for dim 0..100
                let dim_val: u8 = if dim > 100 { 100 } else { dim as u8 };

                Some(LightCommand::CCT(dim_val, ct_val, gm_val))
            } else {
                None
            }
        }
        Some(ModeType::HSI) => {
            if let (Some(hue), Some(sat), Some(dim)) = (state.hue, state.sat, state.dim) {
                // Check range for hue 0..360
                let hue_val: u16 = if hue > 360 { 360 } else { hue };
                // Check range for sat 0..100
                let sat_val: u8 = if sat > 100 { 100 } else { sat as u8 };
                // Check range for dim 0..100
                let dim_val: u8 = if dim > 100 { 100 } else { dim as u8 };

                Some(LightCommand::HSI(hue_val, sat_val, dim_val))
            } else {
                None
            }
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
    println!("Light thread running");
    let mut rf24 = nanlite::rf24_init(config.device.clone(), config.nrf24_ce_gpio).unwrap();
    while let Some((idx, cmd)) = rx.recv().await {
        println!("GOT = {:?}", (idx, cmd));
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
    mut stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
) {
    println!("Thread {} starting", peer.to_string());
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
                    println!("EOF received");
                    break;
                }
                let buf_string = String::from_utf8_lossy(&buf);
                //println!("Received line: {:?}", buf_string);
                let command: Command = serde_json::from_str(&buf_string).unwrap();
                println!("Received message: {:?}", command);
                let update_command = {
                    let mut light_states_mut = light_states.lock().unwrap();
                    update_state(&mut light_states_mut[command.idx as usize], &command.state);
                    update_light(&light_states_mut[command.idx as usize])
                };

                // Send command to light thread.
                println!("Out: {:?}", update_command);
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
                println!("Error receiving message: {}", e);
                break;
            }
        }
    }

    println!("Thread {} finishing", peer.to_string());
}

fn read_config() -> Result<configuration::Configuration, Box<dyn Error>> {
    let config_data = fs::read_to_string("config.json")?;
    Ok(serde_json::from_str(&config_data)?)
}

#[tokio::main]
async fn main() {
    let config = read_config();
    if let Err(err) = config {
        eprintln!("Error loading configuration: {}", err);
        return;
    }
    let config = config.unwrap();

    let addr = config.network.bind_addr.clone();
    let socket = TcpListener::bind(&addr).await.unwrap();

    println!("Listening on {}", addr);

    // Initial light states (unknown).
    let num_lights = config.lights.len();
    let initial_states: Vec<State> = vec![
        State {
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

    // Spawn lights thread.
    let hardware_config = config.hardware.clone();
    tokio::spawn(async move { lights_task(&hardware_config, rx).await });

    while let Ok((stream, peer)) = socket.accept().await {
        let light_states = light_states.clone();
        let tx = tx.clone();
        let light_config = config.lights.clone();
        println!("Incoming connection from: {}", peer.to_string());
        tokio::spawn(async move {
            connection_task(&light_config, light_states, tx, stream, peer).await;
        });
    }
}
