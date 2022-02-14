
use ethers::prelude::*;
use std::time::Duration;

// pub async fn get_ipc_provider(url: &String, duration: u64) -> Provider<Ipc> {
//     let provider = Provider::connect_ipc(url)
//         .await
//         .unwrap()
//         .interval(Duration::from_millis(duration));
//     provider
// }

pub async fn get_ws_provider(url: &String, duration: u64) -> Provider<Ws> {
    let ws = Ws::connect(url)
        .await
        .expect("Can't connect to Websocket Provider");

    Provider::new(ws).interval(Duration::from_millis(duration))
}

pub fn get_http_client(url: &String) -> Provider<Http> {
    Provider::<Http>::try_from(url.clone()).expect("Can't connect to HTTP Provider")
}
