use std::time::Duration;

use anyhow::Ok;
use ethers::prelude::*;
use ethers::types::Transaction;
use ethers::providers::Http;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint = "wss://mainnet.infura.io/ws/v3/c50426cd378c4a0fb803eff92a4d9aed";
    let ws = Ws::connect(endpoint).await?;
    let client = Provider::<Http>::try_from("http://localhost:8545").unwrap();

    println!("endpoint ready?: {}", ws.ready());

    let provider = Provider::new(ws).interval(Duration::from_millis(2000));
    let mut stream = provider.watch_blocks().await?;
    while let Some(block) = stream.next().await {
        // println!("New block {}", &block);
        let full_block = client
            .get_block_with_txs(block)
            .await?
            .expect("oh shit, block probably hasnt arrived");

      println!("New block {}", &full_block.hash.unwrap());

        let uniswap_txns: Vec<&Transaction> = full_block
            .transactions
            .clone()
            .iter()
            .filter(|txn| {
                let is_uniswap_txn: bool = match txn.to {
                    Some(to_address) => {
                        let uniswap_addr = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"
                          .parse::<H160>()
                          .expect("Can't parse string to H160");
                        let to_uniswap = to_address
                            == uniswap_addr;
                        if to_uniswap {
                          println!("{} :: Transaction was to {}, uniswap: {}", to_uniswap, to_address, uniswap_addr);
                        }
                        to_uniswap
                    }

                    None => false,
                };
                is_uniswap_txn
            })
            .collect();


        // println!("New block {}", serde_json::to_string_pretty(&full_block).unwrap());
    }
    Ok(())
}
