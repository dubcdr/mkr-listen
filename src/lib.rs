// Uniswap Constants

// pub const TOKEN_LIST_ENDPOINT: &'static str = "https://defi.cmc.eth.link";
pub const TOKEN_LIST_ENDPOINT: &'static str = "https://tokens.coingecko.com/uniswap/all.json";

pub enum RpcProvider {
    Alchemy,
    Geth,
    Infura,
}

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

pub mod provider {
    use ethers::prelude::*;
    use std::time::Duration;

    pub async fn get_ipc_provider(url: &String, duration: u64) -> Provider<Ipc> {
        let provider = Provider::connect_ipc(url)
            .await
            .unwrap()
            .interval(Duration::from_millis(duration));
        provider
    }

    pub async fn get_ws_provider(url: &String, duration: u64) -> Provider<Ws> {
        let ws = Ws::connect(url)
            .await
            .expect("Can't connect to Websocket Provider");
        let provider = Provider::new(ws).interval(Duration::from_millis(duration));
        provider
    }

    pub fn get_http_client(url: &String) -> Provider<Http> {
        Provider::<Http>::try_from(url.clone()).expect("Can't connect to HTTP Provider")
    }
}

pub mod uni_v2 {
    use std::collections::HashMap;
    use std::sync::Arc;

    use anyhow::{Ok, Result};
    use ethers::prelude::*;
    use ethers::utils::hex;
    use rayon::prelude::*;
    use token_list::Token;

    pub const UNISWAP_ADDR_STR: &'static str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
    pub const AVAILABLE_METHOD_STRS: &'static [&'static str] = &[
        "0x18cbafe5", // SwapExactTokensForEth(uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)
        "0x38ed1739", // SwapExactTokensForTokens(uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)
        "0x8803dbee", // SwapTokensForExactTokens(uint256 amountOut, uint256 amountInMax, address[] path, address to, uint256 deadline)
        "0x4a25d94a", // SwapTokensForExactEth(uint256 amountOut, uint256 amountInMax, address[] path, address to, uint256 deadline)
        "0x7ff36ab5", // SwapExactEthforTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
        "0xfb3bdb41", // SwapEthforExactTokens(uint256 amountOut, address[] path, address to, uint256 deadline)
    ];

    type ISwapEthInputs = (U256, Vec<Address>, Address, U256);
    type ISwapTokenInputs = (U256, U256, Vec<Address>, Address, U256);

    enum UniTxnInput {
        SwapEth(ISwapEthInputs),
        SwapToken(ISwapTokenInputs),
    }

    #[derive(PartialEq)]
    enum UniTxnMethod {
        Eth(EthTxnMethod),
        Token(TokenTxnMethod),
    }

    #[derive(PartialEq)]
    enum EthTxnMethod {
        SwapEthForExactTokens,
        SwapExactEthForTokens,
    }

    #[derive(PartialEq)]
    enum TokenTxnMethod {
        SwapExactTokensForEth,
        SwapExactTokensForTokens,
        SwapTokensForExactTokens,
        SwapTokensForExactEth,
    }

    pub struct UniTxnInputs {
        origin_address: Option<Address>,
        origin_amount: U256,
        destination_address: Option<Address>,
        destination_amount: U256,
    }

    impl UniTxnInputs {
        pub fn new<T>(
            txn: &Transaction,
            uniswap_router_contract: &IUniswapV2Router<Provider<T>>,
        ) -> UniTxnInputs
        where
            T: JsonRpcClient,
        {
            let (txn_inputs, method) = decode_txn_inputs(txn, uniswap_router_contract).unwrap();
            match txn_inputs {
                UniTxnInput::SwapEth(inputs) => {
                    let origin_address = None;
                    let destination_address = Some(inputs.1.last().unwrap().clone());
                    let destination_amount = inputs.0;
                    let origin_amount = txn.value;
                    Self {
                        origin_amount,
                        origin_address,
                        destination_amount,
                        destination_address,
                    }
                }
                UniTxnInput::SwapToken(inputs) => match method {
                    UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForTokens)
                    | UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForEth) => {
                        let destination_address: Option<Address> = match method {
                            UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForEth) => None,
                            UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForTokens) => {
                                Some(inputs.2.last().unwrap().clone() as Address)
                            }
                            _ => panic!("Hit an impossible catch all"),
                        };

                        let origin_address: Option<Address> = match method {
                            UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForTokens)
                            | UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForEth) => {
                                Some(inputs.2.get(0).unwrap().clone() as Address)
                            }
                            _ => panic!("Hit an impossible catch all"),
                        };

                        Self {
                            destination_address,
                            destination_amount: inputs.1,
                            origin_address,
                            origin_amount: inputs.0,
                        }
                    }
                    UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactTokens)
                    | UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactEth) => {
                        let destination_address: Option<Address> = match method {
                            UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactEth) => None,
                            UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactTokens) => {
                                Some(inputs.2.last().unwrap().clone() as Address)
                            }
                            _ => panic!("Hit an impossible catch all"),
                        };

                        let origin_address: Option<Address> = match method {
                            UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactTokens)
                            | UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactEth) => {
                                Some(inputs.2.get(0).unwrap().clone() as Address)
                            }
                            _ => panic!("Hit an impossible catch all"),
                        };

                        Self {
                            destination_address,
                            destination_amount: inputs.0,
                            origin_address,
                            origin_amount: inputs.1,
                        }
                    }
                    _ => panic!("We failed"),
                },
            }
        }

        pub fn log_str(&self, token_map: &HashMap<String, Token>) -> String {
            let origin_str =
                UniTxnInputs::build_side_str(&self.origin_amount, &self.origin_address, &token_map);
            let destination_str = UniTxnInputs::build_side_str(
                &self.destination_amount,
                &self.destination_address,
                &token_map,
            );

            format!("Swap {} for {}", origin_str, destination_str)
        }

        fn build_side_str(
            amount: &U256,
            address: &Option<Address>,
            token_map: &HashMap<String, Token>,
        ) -> String {
            match address {
                Some(a) => {
                    let address = format!("0x{}", hex::encode(a));
                    match token_map.get(&address) {
                        Some(t) => {
                            format!(
                                "{} {}",
                                UniTxnInputs::parse_u256_to_f64(amount, t.decimals as usize),
                                t.symbol
                            )
                            // todo format amount
                        }
                        None => {
                            format!("{} {}", amount, address)
                        }
                    }
                }
                None => UniTxnInputs::build_eth_str(amount),
            }
        }

        fn build_eth_str(amount: &U256) -> String {
            format!("{} ETH", UniTxnInputs::parse_u256_to_f64(amount, 18))
        }

        fn parse_u256_to_f64(amount: &U256, decimals: usize) -> f64 {
            // todo handle overflow casting to u128
            let mut padded_str =
                format!("{:0decimals$}", amount.as_u128(), decimals = decimals + 1);

            padded_str.insert(padded_str.len() - decimals, '.');

            let floating = padded_str.parse::<f64>().unwrap();
            floating
        }
    }

    abigen!(
        IUniswapV2Router,
        "./uniswap-v2-abi.json",
        event_derives(serde::Deserialize, serde::Serialize)
    );

    pub fn get_uniswap_router_contract<T>(client: Arc<Provider<T>>) -> IUniswapV2Router<Provider<T>>
    where
        T: JsonRpcClient,
    {
        let address = UNISWAP_ADDR_STR
            .parse::<Address>()
            .expect("Can't find uniswap address");
        IUniswapV2Router::new(address, client.clone())
    }

    pub fn filter_uni_txns(full_block: &Block<Transaction>) -> Vec<&Transaction> {
        full_block
            .transactions
            .par_iter()
            .filter(|txn| {
                // filters if uniswap is to address,
                // filters if method is one we can handle
                let is_uniswap_txn: bool = match txn.to {
                    Some(to_address) => {
                        let uniswap_addr = UNISWAP_ADDR_STR
                            .parse::<H160>()
                            .expect("Can't parse string to H160");
                        let to_uniswap = to_address == uniswap_addr;
                        to_uniswap && AVAILABLE_METHOD_STRS.contains(&&txn.input.to_string()[0..10])
                    }
                    None => false,
                };
                is_uniswap_txn
            })
            .collect()
    }

    pub fn decode_uni_txn_call_data(
        txn: &Transaction,
        contract: IUniswapV2Router<Provider<Http>>,
    ) -> UniTxnInputs {
        UniTxnInputs::new(txn, &contract)
    }

    fn decode_txn_method(txn: &Transaction) -> Option<UniTxnMethod> {
        let method_str = &txn.input.to_string()[0..10];
        match method_str {
            "0x7ff36ab5" => Some(UniTxnMethod::Eth(EthTxnMethod::SwapExactEthForTokens)),
            "0xfb3bdb41" => Some(UniTxnMethod::Eth(EthTxnMethod::SwapEthForExactTokens)),
            "0x18cbafe5" => Some(UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForEth)),
            "0x38ed1739" => Some(UniTxnMethod::Token(
                TokenTxnMethod::SwapExactTokensForTokens,
            )),
            "0x8803dbee" => Some(UniTxnMethod::Token(
                TokenTxnMethod::SwapTokensForExactTokens,
            )),
            "0x4a25d94a" => Some(UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactEth)),
            _ => None,
        }
    }

    fn decode_txn_inputs<T>(
        txn: &Transaction,
        uniswap_router_contract: &IUniswapV2Router<Provider<T>>,
    ) -> Result<(UniTxnInput, UniTxnMethod)>
    where
        T: JsonRpcClient,
    {
        let txn_method = decode_txn_method(&txn).expect("Trying to decode an unsupported method");

        let txn_inputs = match txn_method {
            UniTxnMethod::Eth(EthTxnMethod::SwapExactEthForTokens) => UniTxnInput::SwapEth(
                uniswap_router_contract
                    .decode("swapExactETHForTokens", &txn.input)
                    .unwrap(),
            ),
            UniTxnMethod::Eth(EthTxnMethod::SwapEthForExactTokens) => UniTxnInput::SwapEth(
                uniswap_router_contract
                    .decode("swapETHForExactTokens", &txn.input)
                    .unwrap(),
            ),
            UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForEth) => UniTxnInput::SwapToken(
                uniswap_router_contract
                    .decode("swapExactTokensForETH", &txn.input)
                    .unwrap(),
            ),
            UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForTokens) => {
                UniTxnInput::SwapToken(
                    uniswap_router_contract
                        .decode("swapExactTokensForTokens", &txn.input)
                        .unwrap(),
                )
            }
            UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactTokens) => {
                UniTxnInput::SwapToken(
                    uniswap_router_contract
                        .decode("swapTokensForExactTokens", &txn.input)
                        .unwrap(),
                )
            }
            UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactEth) => UniTxnInput::SwapToken(
                uniswap_router_contract
                    .decode("swapTokensForExactETH", &txn.input)
                    .unwrap(),
            ),
        };

        Ok((txn_inputs, txn_method))
    }
}
