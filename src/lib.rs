// Uniswap Constants

//Infura Constants
pub const INFURA_WS_ENDPOINT: &'static str =
  "wss://mainnet.infura.io/ws/v3";
pub const INFURA_HTTP_ENDPOINT: &'static str =
  "https://mainnet.infura.io/v3";

// Alchemy Constants
// pub const ALCHEMY_HTTP_ENDPOINT: &'static str =
//     "https://eth-mainnet.alchemyapi.io/v2";
// pub const ALCHEMY_WS_ENDPOINT: &'static str =
//     "wss://eth-mainnet.alchemyapi.io/v2";

// Geth Constants
// pub const GETH_HTTP_ENDPOINT: &'static str = "http://localhost:8545";

pub enum RpcProvider {
  Alchemy,
  Geth,
  Infura
}

pub mod uni_v2 {
  use ethers::prelude::*;
  use rayon::prelude::*;
  use std::sync::{Arc, Mutex};
  use ethers::utils::format_ether;
  use paris::Logger;

  pub const UNISWAP_ADDR_STR: &'static str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
  pub const AVAILABLE_METHOD_STRS: &'static [&'static str] = &[
    // "0x18cbafe5", // swapExactTokensForETH(uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)
    // "0x38ed1739", // swapExactTokensForTokens(uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)
    "0x7ff36ab5", // swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
    // "0xfb3bdb41", // swapETHForExactTokens(uint256 amountOut, address[] path, address to, uint256 deadline)
    // "0x8803dbee", // swapTokensForExactTokens(uint256 amountOut, uint256 amountInMax, address[] path, address to, uint256 deadline)
    // "0x4a25d94a", // swapTokensForExactETH(uint256 amountOut, uint256 amountInMax, address[] path, address to, uint256 deadline)
  ];

  abigen!(
    IUniswapV2Router,
    "./uniswap-v2-abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
  );

  pub fn get_uniswap_router_contract(client: Arc<Provider<Http>>) -> IUniswapV2Router<Provider<Http>> {
    let address = UNISWAP_ADDR_STR.parse::<Address>().expect("Can't find uniswap address");
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

  pub fn parallel_decode_uni_txns_call_data(txns: Vec<&Transaction>, client: Arc<Provider<Http>>) {
    let uni_router_contract = get_uniswap_router_contract(client);

    let logger = Logger::new();
    let logger_ref = Arc::new(Mutex::new(logger));

    txns.par_iter().for_each(|txn| {
      let inputs: Result<(U256, Vec<Address>, Address, U256), AbiError> =
        // swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
        uni_router_contract.decode("swapExactETHForTokens", &txn.input);

      let txn_message = format!("txn :: {}", &txn.hash());
      match inputs {
        Ok(inputs) => {
          // let paths: Vec<Address> = inputs.1;
          // for path in paths {
          //     logger
          //         .indent(2)
          //         .log(format!("through: {}", path.to_string()));
          // }
          let logger = Arc::clone(&logger_ref);
          let mut logger = logger.lock().unwrap();
          logger
            .indent(1)
            .log(txn_message)
            .indent(2)
            .log(format!("swap {} ethereum", format_ether(txn.value)))
            .indent(2)
            .log(format!("amountOutMin: {}", inputs.0))
            .indent(2)
            .log(format!("to: {}", inputs.2));
        }
        Err(_err) => {
          let logger = Arc::clone(&logger_ref);
          let mut logger = logger.lock().unwrap();
          logger
            .indent(1)
            .log(txn_message)
            .indent(2)
            .log("Unsupported Uniswap Method");
          // .same()
          // .log(format!("[{}]", err));
        }
      };
    });
  }

  pub fn serial_decode_uni_txns_call_data(txns: Vec<&Transaction>, client: Arc<Provider<Http>>) {
    let uni_router_contract = get_uniswap_router_contract(client);
    let mut logger = Logger::new();

    for txn in txns {
        logger.indent(1).log(format!("Txn :: {}", &txn.hash()));
        let inputs: Result<(U256, Vec<Address>, Address, U256), AbiError> =
        // swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
          uni_router_contract.decode("swapExactETHForTokens", &txn.input);
        match inputs {
            Ok(inputs) => {
                // swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
                let paths: Vec<Address> = inputs.1;
                logger
                    .indent(2)
                    .log(format!("swap {} ethereum", txn.value))
                    .indent(2)
                    .log(format!("amountOutMin: {}", inputs.0))
                    .indent(2)
                    .log(format!("to: {}", inputs.2));
                // logger.log(format!("path: ${}", inputs.1));
                for path in paths {
                    logger
                        .indent(2)
                        .log(format!("through: {}", path.to_string()));
                }
            }
            Err(_err) => {
                logger
                    .indent(2)
                    .log("Unsupported Uniswap Method");
                    // .same()
                    // .log(format!("[{}]", err));
            }
        };
    }
  }

  fn _decode_uni_txn_call_data(_txn: &Transaction) {

  }

}
