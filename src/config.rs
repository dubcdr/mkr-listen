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
        pub use_ipc: bool,
        pub ws_url: String,
        pub http_url: String,
    }

    pub fn get_config() -> UniListenConfig {
        dotenv().ok();

        let ws_provider_opt = Arg::new("ws-provider")
            .long("ws-provider")
            .default_value("infura")
            .group("ws");
        let ws_provider_id_opt = Arg::new("ws-provider-id")
            .long("ws-provider-id")
            .group("ws");
        let use_ipc_opt = Arg::new("use-ipc")
            .long("use-ipc")
            .group("ws")
            .default_value("false");
        let ipc_location_opt = Arg::new("ipc-location").long("ipc-location").group("ws");
        let http_provider_opt = Arg::new("http-provider")
            .default_value("infura")
            .long("http-provider")
            .group("http");
        let http_provider_id_opt = Arg::new("http-provider-id")
            .long("http-provider-id")
            .group("http");

        let matches = App::new("uni-listen")
            .version("0.1")
            .about("A simple cli app used to watch Uniswap V2 Router")
            .author("Devin Riley")
            .args([
                ws_provider_opt,
                ws_provider_id_opt,
                use_ipc_opt,
                ipc_location_opt,
                http_provider_opt,
                http_provider_id_opt,
            ])
            .get_matches();

        let use_ipc: bool = matches
            .value_of("ws-provider-id")
            .unwrap_or("false")
            .to_lowercase()
            .eq("true");

        let ipc_location = matches.value_of("ipc-location");

        let http_id: Option<String> = match matches.value_of("http-provider-id") {
            Some(str) => Some(str.to_string()),
            None => Some(env::var("HTTP_PROJECT_ID").expect(
                "To use an external provider you must include a project id. See documentation",
            )),
        };

        let ws_id: Option<String> = match use_ipc {
            true => None,
            false => match matches.value_of("ws-provider-id") {
                Some(str) => Some(str.to_string()),
                None => Some(
                    http_id
                        .clone()
                        .unwrap_or(env::var("HTTP_PROJECT_ID").expect(
                            "To use an external provider we need to include a project id.",
                        )),
                ),
            },
        };

        let http_url = match matches
            .value_of("http-provider")
            .unwrap()
            .to_lowercase()
            .as_str()
        {
            "infura" => format!("{}/{}", INFURA_HTTP_ENDPOINT, &http_id.unwrap()),
            "alchemy" => format!("{}/{}", ALCHEMY_HTTP_ENDPOINT, http_id.unwrap()),
            "geth" => format!("{}", GETH_HTTP_ENDPOINT),
            _ => panic!("Unsupported http provider."),
        };

        let ws_url = match matches
            .value_of("ws-provider")
            .unwrap()
            .to_lowercase()
            .as_str()
        {
            "infura" => format!("{}/{}", INFURA_WS_ENDPOINT, ws_id.unwrap()),
            "alchemy" => format!("{}/{}", ALCHEMY_WS_ENDPOINT, ws_id.unwrap()),
            "geth" => match ipc_location {
                None => format!("{}", GETH_HTTP_ENDPOINT),
                Some(l) => format!("{}", l),
            },
            _ => panic!("Unsupported WS provider"),
        };

        UniListenConfig {
            http_url,
            ws_url,
            use_ipc,
        }
    }
}
