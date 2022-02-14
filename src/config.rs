use std::env;

use clap::{App, Arg};
use dotenv::dotenv;

//Infura Constants
const INFURA_WS_ENDPOINT: &str = "wss://mainnet.infura.io/ws/v3";
const INFURA_HTTP_ENDPOINT: &str = "https://mainnet.infura.io/v3";

// Alchemy Constants
// pub const ALCHEMY_HTTP_ENDPOINT: &str = "https://eth-mainnet.alchemyapi.io/v2";
// pub const ALCHEMY_WS_ENDPOINT: &str = "wss://eth-mainnet.alchemyapi.io/v2";

// Geth Constants
// pub const GETH_HTTP_ENDPOINT: &str = "http:H//localhost:8545";

pub struct UniListenConfig {
    pub ws_url: String,
    pub http_url: String,
    pub since_block: Option<u64>,
    pub prev_blocks: Option<u32>,
    pub watch_blocks: bool,
}

pub fn get_config() -> UniListenConfig {
    dotenv().ok();

    let ws_provider_opt = Arg::new("ws-provider-url")
        .long("ws-provider-url")
        .default_value(INFURA_WS_ENDPOINT)
        .group("provider");
    let http_provider_opt = Arg::new("http-provider-url")
        .default_value(INFURA_HTTP_ENDPOINT)
        .long("http-provider-url")
        .group("provider");

    let since_block_opt = Arg::new("since-block")
        .long("since-block")
        .takes_value(true)
        .group("time-machine");
    let include_prev_blocks = Arg::new("prev-blocks")
        .long("include-prev-n-blocks")
        .takes_value(true)
        .group("time-machine");
    let watch_new_blocks = Arg::new("watch-blocks")
        .long("watch-new-blocks")
        .default_value("true")
        .group("time-machine");

    let matches = App::new("uni-listen")
        .version("0.1")
        .about("A simple cli app used to watch Uniswap V2 Router")
        .author("Devin Riley")
        .args([
            ws_provider_opt,
            http_provider_opt,
            since_block_opt,
            include_prev_blocks,
            watch_new_blocks,
        ])
        .get_matches();

    let http_id: Option<String> = match env::var("HTTP_PROJECT_ID") {
        Ok(s) => Some(s),
        Err(_) => None,
    };

    let ws_id: Option<String> = match env::var("WS_PROJECT_ID") {
        Ok(s) => Some(s),
        Err(_) => http_id.clone(),
    };

    let build_url = |url: String, id: Option<String>| match id {
        Some(id) => format!("{}/{}", url, id),
        None => url,
    };

    let http_url = build_url(
        matches
            .value_of("http-provider-url")
            .unwrap()
            .to_lowercase(),
        http_id,
    );

    let ws_url = build_url(
        matches.value_of("ws-provider-url").unwrap().to_lowercase(),
        ws_id,
    );

    let since_block = matches.value_of("since-block").map(|s| {
        s.parse::<u64>()
            .expect("--since-block format must be a u64")
    });

    let prev_blocks = matches.value_of("prev-blocks").map(|s| {
        s.parse::<u32>()
            .expect("--include-prev-n-blocks format must be a u32")
    });

    if since_block.is_some() && prev_blocks.is_some() {
        panic!("Can't set --since-block and --include-prev-n-blocks");
    }

    let watch_blocks = match matches.value_of("watch-blocks") {
        Some(s) => match s.to_lowercase().as_str() {
            "true" => true,
            "false" => false,
            _ => panic!("--watch-blocks must be \"true\" or \"false\""),
        },
        None => true,
    };

    UniListenConfig {
        watch_blocks,
        http_url,
        ws_url,
        prev_blocks,
        since_block,
    }
}
