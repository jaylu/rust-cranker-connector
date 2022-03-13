use std::env;

use async_tungstenite::async_std::connect_async;
use futures::StreamExt;

async fn connect_to_router() {
    let result = connect_async("wss://localhost:16488/register?connectorId=abc").await;
    match result {
        Ok((stream, response)) => {
            let (write, read) = stream.split();
            read.for_each(|message| async {
                let data = message.unwrap().into_data();
            });
        }
        Err(e) => {
            todo!()
        }
    }
}

#[tokio::main]
async fn main() {
    connect_to_router().await
}
