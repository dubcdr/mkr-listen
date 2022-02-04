// Uniswap Constants

//Infura Constants
pub const INFURA_WS_ENDPOINT: &'static str = "wss://mainnet.infura.io/ws/v3";
pub const INFURA_HTTP_ENDPOINT: &'static str = "https://mainnet.infura.io/v3";

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
  Infura,
}

pub mod uni_v2 {
  use ethers::prelude::*;
  use ethers::utils::format_ether;
  use paris::Logger;
  use rayon::prelude::*;
  use std::sync::{Arc, Mutex};

  pub const UNISWAP_ADDR_STR: &'static str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
  pub const AVAILABLE_METHOD_STRS: &'static [&'static str] = &[
    // "0x18cbafe5", // SwapExactTokensForEth(uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)
    // "0x38ed1739", // SwapExactTokensForTokens(uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)
    // "0x8803dbee", // SwapTokensForExactTokens(uint256 amountOut, uint256 amountInMax, address[] path, address to, uint256 deadline)
    // "0x4a25d94a", // SwapTokensForExactEth(uint256 amountOut, uint256 amountInMax, address[] path, address to, uint256 deadline)
    "0x7ff36ab5", // SwapExactEthforTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
    // "0xfb3bdb41", // SwapEthforExactTokens(uint256 amountOut, address[] path, address to, uint256 deadline)
  ];

  type ISwapEthInputs = (U256, Vec<Address>, Address, U256);
  type ISwapTokenInputs = (U256, U256, Vec<Address>, Address, U256);

  enum UniTxnInput {
    SwapEth(ISwapEthInputs)
    // SwapToken
  }

  enum UniTxnMethod {
    // SwapEthForExactTokens,
    SwapExactEthForTokens,
    // SwapExactTokensForEth,
    // SwapExactTokensForTokens,
    // SwapTokensForExactTokens,
    // SwapTokensForExactEth,
  }

  abigen!(
      IUniswapV2Router,
      "./uniswap-v2-abi.json",
      event_derives(serde::Deserialize, serde::Serialize)
  );

  pub fn get_uniswap_router_contract(
    client: Arc<Provider<Http>>,
  ) -> IUniswapV2Router<Provider<Http>> {
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

  pub fn parallel_decode_uni_txns_call_data(
    txns: Vec<&Transaction>,
    client: Arc<Provider<Http>>,
  ) {
    let uni_router_contract = get_uniswap_router_contract(client);

    let logger = Logger::new();
    let logger = Arc::new(Mutex::new(logger));

    txns.par_iter().for_each(|txn| {
      let (txn_inputs, txn_method) =
        decode_txn_inputs(&txn, &uni_router_contract)
          .expect("Transactions should be filtered by decode step");

      match txn_inputs {
        UniTxnInput::SwapEth(inputs) => log_eth_txn_inputs(&txn, &txn_method, &inputs, &logger)
      }
    });
  }

  fn decode_txn_method(txn: &Transaction) -> Option<UniTxnMethod> {
    let method_str = &txn.input.to_string()[0..10];
    match method_str {
      "0x7ff36ab5" => Some(UniTxnMethod::SwapExactEthForTokens),
      _ => None,
    }
  }

  fn decode_txn_inputs(
    txn: &Transaction,
    uniswap_router_contract: &IUniswapV2Router<Provider<Http>>
  ) -> Result<(UniTxnInput, UniTxnMethod), AbiError> {
    let txn_method = decode_txn_method(&txn)
      .expect("Trying to decode an unsupported method");

    let txn_inputs = match txn_method {
      UniTxnMethod::SwapExactEthForTokens => UniTxnInput::SwapEth(uniswap_router_contract.decode("swapExactETHForTokens", &txn.input).unwrap())
    };

    Ok((txn_inputs, txn_method))

  }

  fn log_eth_txn_inputs(
    txn: &Transaction,
    _txn_method: &UniTxnMethod,
    txn_inputs: &ISwapEthInputs,
    logger: &Arc<Mutex<Logger>>
  ) {
    let logger = Arc::clone(&logger);
    let mut logger = logger.lock().unwrap();
    logger
      .indent(1)
      .log(format!("txn :: {}", &txn.hash()))
      .indent(2)
      .log(format!("swap {} ethereum", format_ether(txn.value)))
      .indent(2)
      .log(format!("amountOutMin: {}", txn_inputs.0))
      .indent(2)
      .log(format!("to: {}", txn_inputs.2));
  }

  pub fn serial_decode_uni_txns_call_data(txns: Vec<&Transaction>, client: Arc<Provider<Http>>) {
    let uni_router_contract = get_uniswap_router_contract(client);
    let mut logger = Logger::new();

    for txn in txns {
      logger.indent(1).log(format!("Txn :: {}", &txn.hash()));
      let inputs: Result<(U256, Vec<Address>, Address, U256), AbiError> =
        // SwapExactEthforTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
        uni_router_contract.decode("SwapExactEthforTokens", &txn.input);
      match inputs {
        Ok(inputs) => {
          // SwapExactEthforTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
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
          logger.indent(2).log("Unsupported Uniswap Method");
          // .same()
          // .log(format!("[{}]", err));
        }
      };
    }
  }
}
