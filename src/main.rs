use std::time::Duration;

use anyhow::Ok;
use ethers::prelude::*;
use ethers::providers::Http;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint = "wss://mainnet.infura.io/ws/v3/c50426cd378c4a0fb803eff92a4d9aed";
    let ws = Ws::connect(endpoint).await?;
    let client = Provider::<Http>::try_from("http://localhost:8545").unwrap();

    println!("endpoint ready?: {}", ws.ready());

    let provider = Provider::new(ws).interval(Duration::from_millis(2000));
    let mut stream = provider.watch_blocks().await?.take(5);
    while let Some(block) = stream.next().await {
        println!("New block {}", &block);
        let full_block = client
            .get_block_with_txs(block)
            .await?
            .expect("oh shit, block probably hasnt arrived");

        let uniswap_txns: Vec<Transaction> = full_block
            .transactions
            .iter_mut()
            .filter(|txn| {
                let is_uniswap_txn: bool = match txn.to {
                    Some(fromAddress) => {
                        fromAddress
                            == "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852"
                                .parse::<H160>()
                                .unwrap()
                    }

                    None => false,
                };
                is_uniswap_txn
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&full_block).unwrap());
    }
    Ok(())
}
