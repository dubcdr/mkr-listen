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
            .get_block(block)
            .await?
            .expect("oh shit, block probably hasnt arrived");

        println!("{}", serde_json::to_string_pretty(&full_block).unwrap());
    }
    Ok(())
}
