use std::str::FromStr;
use clap::Parser;

use std::thread::sleep;
use std::time::{Duration};
use log::LevelFilter;
use apcstatd::*;

/// Daemon gets statistics data from apcupsd daemon and send to mqtt server
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Opts {
    /// Address to apcupsd server. Example: localhost:3551
    source: String,
    /// Address to destination Mqtt server. Example: anton:1883
    target: String,
    /// logging level: off, error, warn, info, debug, trace
    #[clap(short, long, default_value="info")]
    level: String,
    /// comma separated set of fields to be transferred
    #[clap(short, long, default_value="linev,loadpct,bcharge,timeleft,battv,cumonbatt")]
    fields: String,
}


fn main() {
    let opts: Opts = Opts::parse();

    env_logger::builder().filter_level(
        LevelFilter::from_str(opts.level.as_str()).unwrap()
    ).init();
    let allowed_fields: Vec<&str> = opts.fields.split(",").map(|s| s.trim()).collect();
    log::info!("Field set: {:?}", &allowed_fields);

    let mqtt_client = create_mqtt_client(opts.target);

    loop {
        read_status_text(opts.source.as_str())
            .map(clean_and_split)
            .map(|data| filter_fields(&allowed_fields, data))
            .and_then(|data| send_to_mosquitto(&mqtt_client, data))
            .map(|_| log::info!("Data successfully send to mqtt server"))
            .map_err(|e| log::error!("{}", e))
            .ok();

        sleep(Duration::from_secs(5));
    }
}
