use serde::{Deserialize, Serialize};

/* Protocol:
 * in:
 *   {"idx": n, "state": {"mode": ..., "dim": ..., "ct": ..., "gm": ..., "hue": ..., "sat": ... }}
 * out:
 *   {"response":"err", "error":"..."}
 *   {"response":"ok"}
 *   {"response":"state", "state": {...}}
 */

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ModeType {
    #[serde(rename = "cct")]
    CCT,
    #[serde(rename = "hsi")]
    HSI,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ResponseType {
    #[serde(rename = "err")]
    Err,
    #[serde(rename = "ok")]
    OK,
    #[serde(rename = "state")]
    State,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {
    pub mode: Option<ModeType>,
    pub dim: Option<u16>,
    pub ct: Option<u16>,
    pub gm: Option<i16>,
    pub hue: Option<u16>,
    pub sat: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Command {
    pub idx: u16,
    pub state: State,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    pub response: ResponseType,
    pub error: Option<String>,
    pub state: Option<Vec<State>>,
}
