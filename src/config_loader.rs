use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::process;

const CONFIG: &str = "Hosts.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct Host {
    host: String,
    destination: String,
    tls: Option<Tls>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tls {
    public: String,
    private: String,
}

pub struct Config {
    pub dest_map: HashMap<String, String>,
    pub tls_map: HashMap<String, Option<Tls>>,
}

impl Config {
    pub fn load() -> Self {
        let data = fs::read_to_string(CONFIG).unwrap();

        let hosts: Vec<Host> = serde_json::from_str(&data).unwrap(); //TODO: Unwrap()

        Self::to_map(hosts)
    }

    pub fn to_map(hosts: Vec<Host>) -> Self {
        let mut dest_map: HashMap<String, String> = HashMap::new();
        let mut tls_map: HashMap<String, Option<Tls>> = HashMap::new();

        for host in hosts {
            dest_map.insert(host.host.clone(), host.destination);
            tls_map.insert(host.host, host.tls);
        }

        Config { dest_map, tls_map }
    }

    pub fn create() -> Self {
        let tls = Tls {
            public: "./public.pem".into(),
            private: "./private.pem".into(),
        };

        let host = Host {
            host: "emby.citrusfire.co.uk".into(),
            destination: "192.168.68.100:8096".into(),
            tls: Some(tls),
        };

        let serialized = serde_json::to_string(&host).unwrap();

        let write_handle = fs::write(CONFIG, serialized);

        match write_handle {
            Ok(_) => {
                tracing::info!("Created new {CONFIG}, close and then edit your config file.");
            }
            Err(e) => {
                tracing::error!("Could NOT create {CONFIG}. \n{e}");
            }
        }
        Self::to_map(vec![host])
    }
}
