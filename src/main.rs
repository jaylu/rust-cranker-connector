use async_tungstenite::{async_std::connect_async, tungstenite::Error, tungstenite::Result, WebSocketStream};
use async_tungstenite::async_std;
use async_tungstenite::async_std::ConnectStream;
use async_tungstenite::tungstenite::handshake::client::Response;

async fn connect_to_router() {
    let result = connect_async("wss://localhost:16488/register?connectorId=abc").await;
    match result {
        Ok((stream, response)) => handle_wss_connection(stream, response),
        Err(e) => handle_wss_connection_error(e),
    }
}

fn handle_wss_connection_error(error: Error) -> () {
    todo!()
}

fn handle_wss_connection(stream: WebSocketStream<ConnectStream>, response: Response) -> () {
    let (write, read) = stream.split();
    read.for_each(|message| async {
        let data = message.unwrap().into_data();
        tokio::io::stdout().write_all(&data).await.unwrap();
    });
}

#[tokio::main]
async fn main() {
    connect_to_router().await
}
