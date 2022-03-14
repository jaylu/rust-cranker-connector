use async_native_tls::TlsConnector;
use async_tungstenite::async_std::{connect_async, connect_async_with_config, connect_async_with_tls_connector};
use async_tungstenite::tungstenite::{Error, Message};
use async_tungstenite::tungstenite::handshake::client::{generate_key, Request};

use futures::StreamExt;

async fn connect_to_router() {
    let connector = TlsConnector::new().danger_accept_invalid_certs(true);
    let request = Request::builder()
        .method("GET")
        .header("Host", "localhost")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("CrankerProtocol", "1.0")
        .header("Route", "*")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .uri("wss://localhost:16488/register?connectorId=abc")
        .body(())
        .unwrap();

    let result = connect_async_with_tls_connector(request, Some(connector)).await;
    match result {
        Ok((stream, response)) => {
            println!("stream={:?}, response={:?}", stream, response);
            let (write, read) = stream.split();
            read.for_each(|message| async {
                match message {
                    Ok(msg) => {
                        if (msg.is_text()) {
                            println!("error: {:?}", msg.into_text())
                        }
                    }
                    Err(_) => {}
                }
            });
        }
        Err(e) => {
            eprintln!("error: {:?}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    connect_to_router().await
}
