extern crate core;

use std::time::Duration;

use anyhow::{Ok as AnyhowOk};
use std::env;
use dotenv::dotenv;

use ethers::prelude::*;
use ethers::providers::Http;
use ethers::types::Transaction;
use paris::Logger;
use std::sync::{Arc};
use uni_listen::{INFURA_HTTP_ENDPOINT, INFURA_WS_ENDPOINT};
use uni_listen::uni_v2::{filter_uni_txns, parallel_decode_uni_txns_call_data};


#[tokio::main]
async fn main() -> anyhow::Result<()> {
  dotenv().ok();

  let provider = get_ws_provider(2000).await;
  let mut stream = provider.watch_blocks().await?;

  let client = get_http_client();
  let arc_client = Arc::new(client.clone());

  let mut logger = Logger::new();

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
    if uniswap_txns.len() == 0 {
      logger.indent(1).info("No supported uniswap transactions");
    } else {
      // decode and log
      parallel_decode_uni_txns_call_data(uniswap_txns, arc_client.clone());
    }

    logger.loading("Waiting for next transaction...");
  }

  AnyhowOk(())
}

async fn get_ws_provider(duration: u64) -> Provider<Ws> {
  let infura_project_id = env::var("INFURA_PROJECT_ID").expect("Need infura project id");
  let ws = Ws::connect(format!("{}/{}", INFURA_WS_ENDPOINT, infura_project_id)).await
    .expect("Can't connect to Websocket Provider");
  let provider = Provider::new(ws).interval(Duration::from_millis(duration));
  provider
}

fn get_http_client() -> Provider<Http> {
  let infura_project_id = env::var("INFURA_PROJECT_ID").expect("Need infura project id");
  Provider::<Http>::try_from(format!("{}/{}", INFURA_HTTP_ENDPOINT, infura_project_id)).expect("Can't connect to HTTP Provider")
}
