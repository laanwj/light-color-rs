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
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mqtt {
    pub broker_addr: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,
}

fn default_topic_prefix() -> String {
    "nanlite".to_string()
}

impl Mqtt {
    pub fn broker_host_port(&self) -> (&str, u16) {
        let (host, port) = self
            .broker_addr
            .rsplit_once(':')
            .expect("broker_addr must be in host:port format");
        (
            host,
            port.parse().expect("broker_addr port must be a number"),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Configuration {
    pub network: Network,
    pub hardware: Hardware,
    pub lights: Vec<Light>,
    pub mqtt: Option<Mqtt>,
}
