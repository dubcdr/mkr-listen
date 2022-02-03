extern crate core;

use std::time::Duration;

use anyhow::{Ok as AnyhowOk, Result};
use core::result::Result::Ok;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

use ethers::abi::decode;
use ethers::abi::param_type::ParamType;
use ethers::abi::Error;
use ethers::abi::Token;
use ethers::contract::{AbiError, Contract};
use ethers::prelude::*;
use ethers::providers::Http;
use ethers::types::Transaction;
use paris::Logger;
use std::sync::Arc;

type SwapEthFor = (U256, Vec<Address>, Address, U256);
const UNISWAP_ADDR: &'static str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
const INFURA_WS_ENDPOINT: &'static str =
    "wss://mainnet.infura.io/ws/v3/c50426cd378c4a0fb803eff92a4d9aed";
const ALCHEMY_WS_ENDPOINT: &'static str =
    "wss://eth-mainnet.alchemyapi.io/v2/FV4hMUQL6fF4jqAlk317noVRGY4E9MHl";
const GETH_HTTP_ENDPOINT: &'static str = "http://localhost:8545";
const INFURA_HTTP_ENDPOINT: &'static str =
    "https://mainnet.infura.io/v3/c50426cd378c4a0fb803eff92a4d9aed";
const ALCHEMY_HTTP_ENDPOINT: &'static str =
    "https://eth-mainnet.alchemyapi.io/v2/FV4hMUQL6fF4jqAlk317noVRGY4E9MHl";

abigen!(
    IUniswapV2Router,
    "./uniswap-v2-abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ws = Ws::connect(INFURA_WS_ENDPOINT).await?;

    let client = Provider::<Http>::try_from(ALCHEMY_HTTP_ENDPOINT).unwrap();
    let arc_client = Arc::new(client.clone());

    let address = UNISWAP_ADDR.parse::<Address>()?;
    let contract = IUniswapV2Router::new(address, arc_client.clone());

    let mut logger = Logger::new();
    // logger.info(BANNER);
    logger.loading("Waiting for next transaction...");

    let provider = Provider::new(ws).interval(Duration::from_millis(2000));
    let mut stream = provider.watch_blocks().await?;

    while let Some(block) = stream.next().await {
        let full_block = client
            .get_block_with_txs(block)
            .await?
            .expect("oh shit, block probably hasnt arrived");

        logger.done();
        logger.info(format!("New block {}", &full_block.hash.unwrap()));

        // filter to uniswap transactions
        let uniswap_txns: Vec<&Transaction> = filter_uni_txns(&full_block);

        // decode and log
        // decode_uni_txns(&mut logger, uniswap_txns);
        // uniswap_txns.par_iter().for_each(|txn| {
        //     logger.indent(1).log(format!("Txn :: {}", &txn.hash()));
        //     let inputs: Result<(U256, Vec<Address>, Address, U256), AbiError> =
        //         contract.decode("swapExactETHForTokens", &txn.input);
        //     match inputs {
        //         Ok(inputs) => {
        //             // swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
        //             let paths: Vec<Address> = inputs.1;
        //             logger
        //                 .indent(2)
        //                 .log(format!("swap {} ethereum", txn.value))
        //                 .indent(2)
        //                 .log(format!("amountOutMin: {}", inputs.0))
        //                 .indent(2)
        //                 .log(format!("to: {}", inputs.2));
        //             // logger.log(format!("path: ${}", inputs.1));
        //             for path in paths {
        //                 logger
        //                     .indent(2)
        //                     .log(format!("through: {}", path.to_string()));
        //             }
        //         }
        //         Err(err) => {
        //             logger
        //                 .indent(2)
        //                 .log("Unsupported Uniswap Method")
        //                 .same()
        //                 .log(format!("[{}]", err));
        //         }
        //     };
        // });
        for txn in uniswap_txns {
            logger.indent(1).log(format!("Txn :: {}", &txn.hash()));
            let inputs: Result<(U256, Vec<Address>, Address, U256), AbiError> =
                contract.decode("swapExactETHForTokens", &txn.input);
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
                Err(err) => {
                    logger
                        .indent(2)
                        .log("Unsupported Uniswap Method")
                        .same()
                        .log(format!("[{}]", err));
                }
            };
        }

        // println!("New block {}", serde_json::to_string_pretty(&full_block).unwrap());
        logger.loading("Waiting for next transaction...");
    }

    AnyhowOk(())
}

fn filter_uni_txns(full_block: &Block<Transaction>) -> Vec<&Transaction> {
    full_block
        .transactions
        .par_iter()
        .filter(|txn| {
            let is_uniswap_txn: bool = match txn.to {
                Some(to_address) => {
                    let uniswap_addr = UNISWAP_ADDR
                        .parse::<H160>()
                        .expect("Can't parse string to H160");
                    let to_uniswap = to_address == uniswap_addr;
                    to_uniswap
                }
                None => false,
            };
            is_uniswap_txn
        })
        .collect()
}
