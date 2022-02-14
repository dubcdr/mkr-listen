use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Ok, Result};
use ethers::prelude::*;
use ethers::utils::hex;
use rayon::prelude::*;
use token_list::Token;

use crate::uni_v2_router::UniV2Router;

pub const UNISWAP_ADDR_STR: &str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
pub const AVAILABLE_METHOD_STRS: &[&str] = &[
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
        uniswap_router_contract: &UniV2Router<Provider<T>>,
    ) -> UniTxnInputs
    where
        T: JsonRpcClient,
    {
        let (txn_inputs, method) = decode_txn_inputs(txn, uniswap_router_contract).unwrap();
        match txn_inputs {
            UniTxnInput::SwapEth(inputs) => {
                let origin_address = None;
                let destination_address = Some(*inputs.1.last().unwrap());
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
                            Some(*inputs.2.last().unwrap() as Address)
                        }
                        _ => panic!("Hit an impossible catch all"),
                    };

                    let origin_address: Option<Address> = match method {
                        UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForTokens)
                        | UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForEth) => {
                            Some(*inputs.2.get(0).unwrap() as Address)
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
                            Some(*inputs.2.last().unwrap() as Address)
                        }
                        _ => panic!("Hit an impossible catch all"),
                    };

                    let origin_address: Option<Address> = match method {
                        UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactTokens)
                        | UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactEth) => {
                            Some(*inputs.2.get(0).unwrap() as Address)
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
            UniTxnInputs::build_side_str(&self.origin_amount, &self.origin_address, token_map);
        let destination_str = UniTxnInputs::build_side_str(
            &self.destination_amount,
            &self.destination_address,
            token_map,
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
        let mut padded_str = format!("{:0decimals$}", amount.as_u128(), decimals = decimals + 1);

        padded_str.insert(padded_str.len() - decimals, '.');

        padded_str.parse::<f64>().unwrap()
    }
}

pub fn get_uniswap_router_contract<T>(client: Arc<Provider<T>>) -> UniV2Router<Provider<T>>
where
    T: JsonRpcClient,
{
    let address = UNISWAP_ADDR_STR
        .parse::<Address>()
        .expect("Can't find uniswap address");
    UniV2Router::new(address, client)
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
    uniswap_router_contract: &UniV2Router<Provider<T>>,
) -> Result<(UniTxnInput, UniTxnMethod)>
where
    T: JsonRpcClient,
{
    let txn_method = decode_txn_method(txn).expect("Trying to decode an unsupported method");

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
        UniTxnMethod::Token(TokenTxnMethod::SwapExactTokensForTokens) => UniTxnInput::SwapToken(
            uniswap_router_contract
                .decode("swapExactTokensForTokens", &txn.input)
                .unwrap(),
        ),
        UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactTokens) => UniTxnInput::SwapToken(
            uniswap_router_contract
                .decode("swapTokensForExactTokens", &txn.input)
                .unwrap(),
        ),
        UniTxnMethod::Token(TokenTxnMethod::SwapTokensForExactEth) => UniTxnInput::SwapToken(
            uniswap_router_contract
                .decode("swapTokensForExactETH", &txn.input)
                .unwrap(),
        ),
    };

    Ok((txn_inputs, txn_method))
}
