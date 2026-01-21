use std::io;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use crossterm::event::{self as crossterm_event, Event, KeyCode};
use anyhow::Result;

mod app;
mod ui;
mod event; // This is my local event module, maybe rename or remove if unused? Keep for now.
mod tui;
mod color;

use crate::app::App;

use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::interval;
use tokio::sync::mpsc;
use tokio::io::{AsyncWriteExt, BufReader, AsyncBufReadExt};
use light_protocol::{Command, Response, State};
use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Address of the light server
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    address: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut terminal = tui::init()?;
    let mut app = App::new();

    // Channel for sending commands to network task
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<Command>(32);
    // Channel for receiving updates from network task
    let (tx_update, mut rx_update) = mpsc::channel::<Vec<State>>(32);

    let network_handle = tokio::spawn(async move {
        loop {
            // Try to connect
            let mut stream = match TcpStream::connect(cli.address).await {
                Ok(s) => s,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };
            
            let (reader, mut writer) = stream.split();
            let mut buf_reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                tokio::select! {
                     // Read from socket
                    res = buf_reader.read_line(&mut line) => {
                         match res {
                            Ok(0) => break, // EOF
                            Ok(_) => {
                                // Parse line
                                if let Ok(response) = serde_json::from_str::<Response>(&line) {
                                    if let Some(states) = response.state {
                                        let _ = tx_update.send(states).await;
                                    }
                                }
                                line.clear();
                            }
                            Err(_) => break,
                        }
                    }
                    // Write to socket
                    Some(cmd) = rx_cmd.recv() => {
                        if let Ok(json) = serde_json::to_string(&cmd) {
                            let _ = writer.write_all(json.as_bytes()).await;
                            let _ = writer.write_all(b"\n").await;
                        }
                    }
                }
            }
            // Connection lost, retry...
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let res = run_app(&mut terminal, &mut app, tx_cmd, &mut rx_update).await;

    tui::restore()?;
    network_handle.abort();

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>, 
    app: &mut App,
    tx_cmd: mpsc::Sender<Command>,
    rx_update: &mut mpsc::Receiver<Vec<State>>
) -> Result<()> 
where <B as ratatui::backend::Backend>::Error: Send + Sync + 'static
{
    let mut tick_rate = interval(Duration::from_millis(100));
    
    // Channel for receiving input events
    let (tx_event, mut rx_event) = mpsc::channel(100);
    
    // Spawn blocking task for input
    tokio::task::spawn_blocking(move || {
        loop {
            // Poll for event to allow cancellation
            if crossterm_event::poll(Duration::from_millis(100)).unwrap_or(false) {
                match crossterm_event::read() {
                    Ok(event) => {
                        if tx_event.blocking_send(event).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            } else {
                // Check if channel is closed (receiver dropped)
                if tx_event.is_closed() {
                    break;
                }
            }
        }
    });
    
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        tokio::select! {
             _ = tick_rate.tick() => {
                 // Regular tick
            }
            Some(states) = rx_update.recv() => {
                if app.lights.len() != states.len() {
                    app.lights = states;
                } else {
                    app.lights = states;
                }
            }
            Some(event) = rx_event.recv() => {
                 if let Event::Key(key) = event {
                    if key.kind == crossterm_event::KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            _ => {
                                let old_states = app.lights.clone();
                                app.handle_key_event(key);
                                
                                for idx in &app.selected_indices {
                                    if *idx < app.lights.len() && *idx < old_states.len() {
                                        let new_state = &app.lights[*idx];
                                        let cmd = Command {
                                            idx: *idx as u16,
                                            state: new_state.clone(),
                                        };
                                        let _ = tx_cmd.send(cmd).await;
                                    }
                                }
                            }
                        }
                    }
                 }
            }
        }
    }
}
