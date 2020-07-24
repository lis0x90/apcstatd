#[macro_use] extern crate lazy_static;

use std::collections::HashMap;
use std::io::*;
use std::net::TcpStream;
use std::str::*;
use std::time::{Duration, UNIX_EPOCH};

use log;
use paho_mqtt as mqtt;
use paho_mqtt::{MqttError, Client};
use std::time::SystemTime;
use regex::Regex;
use clap::Clap;
use log::LevelFilter;
use std::thread::sleep;

fn read_status_text(addr: &str) -> Result<String> {
    log::info!("Connect to apcupsd server: {}", addr);
    let mut sock = TcpStream::connect(addr)?;
    log::debug!("Connected");
    sock.write("\x00\x06status".as_bytes())?;

    let mut s = String::new();
    loop {
        let mut buf = [0 as u8; 100];
        if sock.read(&mut buf)? > 0 {
            s.push_str(from_utf8(&buf).unwrap());
        }
        
        if s.contains("  \n\x00\x00") {
            log::debug!("Response received: {}", s);
            return Ok(s)
        }
    }
}

fn strip_field_name(raw: &str) -> String {
    raw.trim_matches(|c| !char::is_alphabetic(c))
        .to_string()
        .to_lowercase()
}

fn strip_field_value(raw: &str) -> String {
    lazy_static! {
        static ref NUMBERIC_VALUE_PATTERN: Regex = Regex::new(r"([\d\.,]+)").unwrap();
    }
    NUMBERIC_VALUE_PATTERN.captures(raw).map(|caps| {
        caps.get(1).map(|m| m.as_str()).unwrap_or(raw)
    }).unwrap_or(raw).to_string()
} 

fn clean_and_split(s: String) -> HashMap<String, String> {
    s.split('\n')
        .map(|s| s.trim())
        .flat_map(|s| match s.splitn(2, ':').collect::<Vec<&str>>().as_slice() {
            [name, value] => Some((strip_field_name(name), strip_field_value(value))),
            _ => None, // todo log about unexpected value string format 
        }).collect()
}

fn filter_fields(data: HashMap<String, String>) -> HashMap<String, String> {
    let allowed = vec!("linev", "loadpct", "bcharge", "timeleft", "battv", "cumonbatt");
    data.into_iter()
        .filter(|(name, _)| 
            allowed.contains(&name.as_str())
        ).collect()
}

fn convert_mqtt_errors(e: MqttError) -> std::io::Error {
    std::io::Error::new(ErrorKind::InvalidData, e.to_string())
}

fn send_to_mosquitto(client: &Client, data: HashMap<String, String>) -> Result<()> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
    data.iter().map(|entry| {
            let msg = mqtt::MessageBuilder::new()
                .topic(format!("/sensors/apcups/{}", entry.0))
                .payload(format!("{}: {}", ts, entry.1))
                .qos(1)
                .finalize();

            log::info!("Send message: {}", msg);
            client.publish(msg)
        }).find(|r| r.is_err())
        .unwrap_or(Ok(()) )
        .map_err(convert_mqtt_errors)
}

fn create_mqtt_client(addr: String) -> Client {
    let options = mqtt::ConnectOptionsBuilder::new()
        .clean_session(false)
        .connect_timeout(Duration::from_secs(5))
        .clean_start(false)
        .mqtt_version(mqtt::MQTT_VERSION_3_1_1)
        .automatic_reconnect(Duration::from_secs(1), Duration::from_secs(60))
        .finalize();

    let mqtt_url = format!("tcp://{}", addr);
    let create_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri(mqtt_url.as_str())
        .client_id("apcstatd")
        .finalize();

    log::info!("Connect to mqtt server: {}", mqtt_url.as_str());
    let mqtt_client = mqtt::Client::new(create_opts)
        .and_then(|mut client| {
            client.set_timeout(Duration::from_secs(5));
            client.connect(options)
                .map(|_| client) // but return client instance instead server
        })
        .unwrap();
    log::debug!("Connected");

    mqtt_client
}

/// Daemon gets statistics data from apcupsd daemon and send to mqtt server
#[derive(Clap)]
#[clap()]
struct Opts {
    /// Address to apcupsd server. Example: localhost:3551
    source: String,
    /// Address to destination Mqtt server. Example: anton:1883
    target: String,
    /// logging level: off, error, warn, info, debug, trace
    #[clap(short, long, default_value="info")]
    level: String,
}

fn main() {
    let opts: Opts = Opts::parse();

    env_logger::builder().filter_level(
        LevelFilter::from_str(opts.level.as_str()).unwrap()
    ).init();

    let mqtt_client = create_mqtt_client(opts.target);

    loop {
        read_status_text(opts.source.as_str())
            .map(clean_and_split)
            .map(filter_fields)
            .and_then(|data| send_to_mosquitto(&mqtt_client, data))
            .map(|_| log::info!("Data successfully send to mqtt server"))
            .map_err(|e| log::error!("{}", e))
            .ok();

        sleep(Duration::from_secs(5));
    }
}
