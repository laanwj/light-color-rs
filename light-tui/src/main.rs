use crate::app::App;
use anyhow::Result;
use clap::Parser;
use crossterm::event::{self as crossterm_event, Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use light_protocol::{Command, Response, State};
use ratatui::Terminal;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::interval;

mod app;
mod color;
mod tui;
mod ui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Address of the light server
    #[arg(short, long, default_value = "127.0.0.1:4983")]
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
    rx_update: &mut mpsc::Receiver<Vec<State>>,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: Send + Sync + 'static,
{
    let mut tick_rate = interval(Duration::from_millis(100));

    let mut event_stream = EventStream::new();

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        tokio::select! {
            _ = tick_rate.tick() => {
                 // Regular tick
            }
            Some(states) = rx_update.recv() => {
                app.lights = states;
                if app.first_connect {
                    app.first_connect = false;
                    // Select all lights on first succesful connect.
                    app.selected_indices = HashSet::from_iter(0..app.lights.len());
                }
            }
            Some(Ok(event)) = event_stream.next() => {
                 if let Event::Key(key) = event {
                    if key.kind == crossterm_event::KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('c') => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    return Ok(());
                                }
                            },
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
