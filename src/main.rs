extern crate core;

use std::time::Duration;

use anyhow::{Ok as AnyhowOk, Result};
use core::result::Result::Ok;

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

abigen!(
    IUniswapV2Router,
    "./uniswap-v2-abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint = "wss://mainnet.infura.io/ws/v3/c50426cd378c4a0fb803eff92a4d9aed";
    let ws = Ws::connect(endpoint).await?;
    const _GETH_SRC: &'static str = "http://localhost:8545";
    const INFURA_SRC: &'static str =
        "https://mainnet.infura.io/v3/c50426cd378c4a0fb803eff92a4d9aed";
    let client = Provider::<Http>::try_from(INFURA_SRC).unwrap();
    let arc_client = Arc::new(client.clone());

    let address = UNISWAP_ADDR.parse::<Address>()?;
    let contract = IUniswapV2Router::new(address, arc_client.clone());

    let mut logger = Logger::new();
    logger.info("Uniswap Ticker");
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
        for txn in uniswap_txns {
            logger.indent(1).log(format!("Txn :: {}", &txn.hash()));
            let inputs: Result<(U256, Vec<Address>, Address, U256), AbiError> =
                contract.decode("swapExactETHForTokens", &txn.input);
            match inputs {
                Ok(inputs) => {
                    // swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
                    logger
                        .log(format!("amountOutMin: ${}", inputs.0))
                        .indent(2)
                        .log(format!("to: ${}", inputs.2));
                    // logger.log(format!("path: ${}", inputs.1));
                }
                Err(err) => {
                    println!("{}", err);
                }
            };
        }

        // println!("New block {}", serde_json::to_string_pretty(&full_block).unwrap());
        logger.loading("Waiting for next transaction...");
    }
    AnyhowOk(())
}

fn decode_uni_txns(logger: &mut Logger, uniswap_txns: Vec<&Transaction>) {
    for txn in uniswap_txns {
        logger.log(format!("Txn :: {}", &txn.hash())).indent(1);

        // swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
        let param_types = [
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Address,
            ParamType::Uint(256),
        ];

        let inputs: Result<Vec<Token>, Error> = decode(&param_types, txn.input.as_ref());

        match inputs {
            Ok(inputs) => {
                for (i, input) in inputs.iter().enumerate() {
                    logger.log(format!("${} :: ${}", i, input)).indent(2);
                }
            }
            Err(_err) => {
                logger
                    .info("Wrong type of transaction")
                    .info("Txn Input")
                    .info(&txn.input);
            }
        }
    }
}

fn filter_uni_txns(full_block: &Block<Transaction>) -> Vec<&Transaction> {
    full_block
        .transactions
        .iter()
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
