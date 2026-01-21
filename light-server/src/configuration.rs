use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Network {
    pub bind_addr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Hardware {
    pub device: String,
    pub nrf24_ce_gpio: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Light {
    pub address: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Configuration {
    pub network: Network,
    pub hardware: Hardware,
    pub lights: Vec<Light>,
}
