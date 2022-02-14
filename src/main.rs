extern crate core;

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

mod config;
mod logging;
mod provider;
mod uni_helpers;
mod uni_v2_router;

use anyhow::Ok as AnyhowOk;
use ethers::prelude::*;
use ethers::types::Transaction;
use paris::Logger;
use token_list::TokenList;
use uni_listen::TOKEN_LIST_ENDPOINT;

use crate::{
    config::get_config,
    logging::log_txns,
    provider::{get_http_client, get_ws_provider},
    uni_helpers::{filter_uni_txns, get_uniswap_router_contract},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let uni_config = get_config();

    let provider = get_ws_provider(&uni_config.ws_url, 2000).await;

    let mut stream = provider.watch_blocks().await?;

    let client = get_http_client(&uni_config.http_url);
    let arc_client = Arc::new(client.clone());

    let mut logger = Logger::new();

    let uni_router_contract = get_uniswap_router_contract(arc_client.clone());

    let mut token_map = HashMap::new();
    let token_list = TokenList::from_uri(TOKEN_LIST_ENDPOINT)
        .await
        .expect("Failed to parse token endpoint");

    for token in token_list.tokens {
        token_map.insert(token.address.clone(), token.clone());
    }

    let current_block = client.get_block_number().await.unwrap();
    let mut starting_block = current_block;

    if uni_config.prev_blocks.is_some() {
        starting_block = starting_block - uni_config.prev_blocks.unwrap();
    } else if uni_config.since_block.is_some() {
        let result: Result<U64, Infallible> = uni_config.since_block.unwrap().try_into();
        let start_block = match result {
            Ok(block) => block,
            Err(_) => panic!("Failed to parse start block correctly"),
        };
        starting_block = start_block;
    }

    while starting_block != current_block {
        let full_block = client.get_block_with_txs(starting_block).await.unwrap();

        match full_block {
            Some(block) => {
                let uniswap_txns: Vec<&Transaction> = filter_uni_txns(&block);

                logger
                    .done()
                    .info(format!("Block {}", &block.hash.unwrap()));
                if !uniswap_txns.is_empty() {
                    log_txns(uniswap_txns, &token_map, &uni_router_contract)
                }
                starting_block = block.number.unwrap() + 1_u64;
            }
            _ => {}
        }
    }

    logger.loading("Waiting for next transaction...");

    if uni_config.watch_blocks {
        while let Some(block) = stream.next().await {
            let full_block = client
                .get_block_with_txs(block)
                .await?
                .expect("oh shit, block probably hasnt arrived");

            // filter to uniswap transactions
            let uniswap_txns: Vec<&Transaction> = filter_uni_txns(&full_block);

            logger
                .done()
                .info(format!("New block {}", &full_block.hash.unwrap()));
            if !uniswap_txns.is_empty() {
                log_txns(uniswap_txns, &token_map, &uni_router_contract)
            }

            logger.loading("Waiting for next transaction...");
        }
    }

    AnyhowOk(())
}
