// pub use univ2router_mod::*;
pub use config::*;

pub mod config {
    use std::env;

    use clap::{App, Arg};
    use dotenv::dotenv;

    //Infura Constants
    pub const INFURA_WS_ENDPOINT: &'static str = "wss://mainnet.infura.io/ws/v3";
    pub const INFURA_HTTP_ENDPOINT: &'static str = "https://mainnet.infura.io/v3";

    // Alchemy Constants
    pub const ALCHEMY_HTTP_ENDPOINT: &'static str = "https://eth-mainnet.alchemyapi.io/v2";
    pub const ALCHEMY_WS_ENDPOINT: &'static str = "wss://eth-mainnet.alchemyapi.io/v2";

    // Geth Constants
    pub const GETH_HTTP_ENDPOINT: &'static str = "http://localhost:8545";

    pub struct UniListenConfig {
        pub ws_url: String,
        pub http_url: String,
    }

    pub fn get_config() -> UniListenConfig {
        dotenv().ok();

        let ws_provider_opt = Arg::new("ws-provider-url")
            .long("ws-provider-url")
            .default_value(INFURA_WS_ENDPOINT)
            .group("ws");
        let http_provider_opt = Arg::new("http-provider-url")
            .default_value("infura")
            .long("http-provider-url")
            .group("http");

        let matches = App::new("uni-listen")
            .version("0.1")
            .about("A simple cli app used to watch Uniswap V2 Router")
            .author("Devin Riley")
            .args([ws_provider_opt, http_provider_opt])
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

        UniListenConfig { http_url, ws_url }
    }
}
