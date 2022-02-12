extern crate core;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Ok as AnyhowOk;
use ethers::prelude::*;
use ethers::types::Transaction;
use paris::Logger;
use token_list::TokenList;

use uni_listen::config::get_config;

use uni_listen::{
    provider::{get_http_client, get_ws_provider},
    uni_helpers::{filter_uni_txns, get_uniswap_router_contract, UniTxnInputs},
    TOKEN_LIST_ENDPOINT,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let uni_config = get_config();

    let provider = get_ws_provider(&uni_config.ws_url, 2000).await;

    // if uni_config.use_ipc {
    //     provider = get_ipc_provider(&uni_config.ws_url, 2000).await;
    // }

    // let provider = get_geth_ws_provider(50).await;
    let mut stream = provider.watch_blocks().await?;

    let client = get_http_client(&uni_config.http_url);
    let arc_client = Arc::new(client.clone());

    let mut logger = Logger::new();

    let uni_router_contract = get_uniswap_router_contract(arc_client.clone());

    let mut token_map = HashMap::new();
    let token_list = TokenList::from_uri(TOKEN_LIST_ENDPOINT)
        .await
        .expect("Failed to parse token endpoint");

    // logger.log("Available Tokens");
    for token in token_list.tokens {
        token_map.insert(token.address.clone(), token.clone());
        // logger.same().log(token.name).indent(1).log(token.address);
    }

    let current_block = client.get_block_number().await.unwrap();
    let mut starting_block = current_block - 50 as u64;

    while starting_block != current_block {
        let full_block = client.get_block_with_txs(starting_block).await.unwrap();

        match full_block {
            Some(block) => {
                let uniswap_txns: Vec<&Transaction> = filter_uni_txns(&block);

                logger
                    .done()
                    .info(format!("New block {}", &block.hash.unwrap()));
                if uniswap_txns.len() > 0 {
                    let call_datas: Vec<(&Transaction, UniTxnInputs)> = uniswap_txns
                        .iter()
                        .map(|txn| {
                            let call_data = UniTxnInputs::new(&txn, &uni_router_contract);
                            (*txn, call_data)
                        })
                        .collect();
                    call_datas.iter().for_each(|(txn, call_data)| {
                        logger.indent(1).log(format!(
                            "Txn {} :: {}",
                            txn.hash,
                            call_data.log_str(&token_map)
                        ));
                    })
                }
                starting_block = block.number.unwrap() + 1 as u64;
            }
            _ => {}
        }
    }

    logger.loading("Waiting for next transaction...");

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
        if uniswap_txns.len() > 0 {
            let call_datas: Vec<(&Transaction, UniTxnInputs)> = uniswap_txns
                .iter()
                .map(|txn| {
                    let call_data = UniTxnInputs::new(&txn, &uni_router_contract);
                    (*txn, call_data)
                })
                .collect();
            call_datas.iter().for_each(|(txn, call_data)| {
                logger.indent(1).log(format!(
                    "Txn {} :: {}",
                    txn.hash,
                    call_data.log_str(&token_map)
                ));
            })
        }

        logger.loading("Waiting for next transaction...");
    }

    AnyhowOk(())
}
