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

// pub const TOKEN_LIST_ENDPOINT: &'static str = "https://defi.cmc.eth.link";
pub const TOKEN_LIST_ENDPOINT: &'static str = "https://tokens.coingecko.com/uniswap/all.json";


pub enum RpcProvider {
  Alchemy,
  Geth,
  Infura,
}

pub mod uni_v2 {
  use std::collections::HashMap;
  use std::sync::{Arc, Mutex};

  use ethers::prelude::*;
  use ethers::utils::{format_ether, hex};
  use paris::Logger;
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

  impl UniTxnMethod {
    fn eth(self) -> EthTxnMethod {
      if let UniTxnMethod::Eth(e) = self {
        e
      } else {
        panic!("Not a EthTxnMethod")
      }
    }

    fn token(self) -> TokenTxnMethod {
      if let UniTxnMethod::Token(t) = self {
        t
      } else {
        panic!("Not a TokenTxnMethod")
      }
    }
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
    token_map: &HashMap<String, Token>,
  ) {
    let uni_router_contract = get_uniswap_router_contract(client);

    let logger = Logger::new();
    let logger = Arc::new(Mutex::new(logger));

    txns.par_iter().for_each(|txn| {
      let (txn_inputs, txn_method) = decode_txn_inputs(&txn, &uni_router_contract)
        .expect("Transactions should be filtered by decode step");

      match txn_inputs {
        UniTxnInput::SwapEth(inputs) => {
          let txn_method: EthTxnMethod = UniTxnMethod::eth(txn_method);
          log_eth_txn_inputs(&txn, &txn_method, &inputs, &logger, token_map)
        }
        UniTxnInput::SwapToken(inputs) => {
          let txn_method: TokenTxnMethod = UniTxnMethod::token(txn_method);
          log_token_txn_inputs(&txn, &txn_method, &inputs, &logger, token_map)
        }
      }
    });
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

  fn decode_txn_inputs(
    txn: &Transaction,
    uniswap_router_contract: &IUniswapV2Router<Provider<Http>>,
  ) -> Result<(UniTxnInput, UniTxnMethod), AbiError> {
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

  fn log_eth_txn_inputs(
    txn: &Transaction,
    txn_method: &EthTxnMethod,
    txn_inputs: &ISwapEthInputs,
    logger: &Arc<Mutex<Logger>>,
    token_map: &HashMap<String, Token>,
  ) {
    let token_addr = txn_inputs.2;
    let amount_out_str = match get_token(token_map, &token_addr) {
      Some(t) => {
        // let units = txn_inputs.0 / t.decimals;
        let units = quant_after_division(txn_inputs.0, &t);
        let symbol = &t.symbol;
        format!("for {} {}", units, symbol)
      },
      None => {
        format!("for {} {}", txn_inputs.0, &token_addr)
      }
    };

    let logger = Arc::clone(&logger);
    let mut logger = logger.lock().unwrap();
    logger
      .same()
      .indent(1)
      .log(format!("Txn {} :: ", &txn.hash()))
      .log(format!("Trade {} ETH {}", format_ether(txn.value), amount_out_str));
  }

  fn log_token_txn_inputs(
    txn: &Transaction,
    method: &TokenTxnMethod,
    inputs: &ISwapTokenInputs,
    logger: &Arc<Mutex<Logger>>,
    token_map: &HashMap<String, Token>,
  ) {
    let origin_token_addr = inputs.2.get(0).unwrap();
    let origin_token = get_token(token_map, origin_token_addr);
    let origin_quantity = inputs.0;
    let origin_token_str = match origin_token {
      Some(t) => {
        format!("{}", &t.symbol)
      },
      None => {
        format!("{}", origin_token_addr)
      }
    };
    let origin_token_quantity = match origin_token {
      Some(t) => quant_after_division(origin_quantity, &t),
      None => origin_quantity.as_u128()
    };

    let destination_token_addr = inputs.2.last().unwrap();
    let destination_token = get_token(token_map, destination_token_addr);
    let destination_quantity = inputs.1;
    let destination_token_str = match method {
      TokenTxnMethod::SwapExactTokensForEth | TokenTxnMethod::SwapTokensForExactEth => String::from("ETH"),
      TokenTxnMethod::SwapTokensForExactTokens | TokenTxnMethod::SwapExactTokensForTokens => match destination_token {
        Some(t) => format!("{}", &t.symbol),
        None => format!("{}", origin_token_addr)
      }
    };
    let destination_token_quantity = match method {
      TokenTxnMethod::SwapExactTokensForEth | TokenTxnMethod::SwapTokensForExactEth => format_ether(destination_quantity).as_u128(),
      TokenTxnMethod::SwapTokensForExactTokens | TokenTxnMethod::SwapExactTokensForTokens => match destination_token {
        Some(t) => quant_after_division(destination_quantity, &t),
        None => destination_quantity.as_u128()
      }
    };

    let input_str = format!("Trade {} {} for {} {}", origin_token_quantity, origin_token_str, destination_token_quantity, destination_token_str);
    let logger = Arc::clone(&logger);
    let mut logger = logger.lock().unwrap();
    logger.same()
      .indent(1)
      .log(format!("Txn {} :: ", &txn.hash()))
      .log(input_str);
  }

  fn quant_after_division(origin_quantity: U256, t: &&Token) -> u128 {
    origin_quantity.as_u128() / ((10 as u128).pow(*&t.decimals as u32))
  }

  fn get_token<'a>(token_map: &'a HashMap<String, Token>, token_addr: &Address) -> Option<&'a Token> {
    let token_addr_str = format!("0x{}",hex::encode(token_addr));
    token_map.get(&token_addr_str)
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
            .same()
            .indent(2)
            .log(format!("swap {} eth", format_ether(txn.value)))
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
