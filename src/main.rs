use async_native_tls::TlsConnector;
use async_tungstenite::async_std::{
    connect_async, connect_async_with_config, connect_async_with_tls_connector,
};
use async_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use async_tungstenite::tungstenite::{Error, Message};

use futures::StreamExt;

fn parse(input: &str) {
    let split = input.split("\n");
    let method
    split.for_each(|item| {

    })

}

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
            // println!("stream={:?}, response={:?}", stream, response);
            let (write, read) = stream.split();
            let reading = read.for_each(|message| async {
                match message {
                    Ok(msg) => {
                        if (msg.is_text()) {
                            // construct http reqeust
                            let text_line = msg.into_text().unwrap();
                            println!("text: {:?}", text_line);
                            if text_line.ends_with("_1") {
                                // has body
                            } else if text_line.ends_with("_2") {
                                // no body
                                println!("no_body");
                            } else if text_line.ends_with("_3") {
                                // request finish
                                println!("no_body_no_content_length");
                            }

                            // let println!("text: {:?}", msg.into_text());
                        } else if (msg.is_binary()) {
                            println!("binary_text: {:?}", msg.into_text());
                            // continue send http body
                        } else if (msg.is_close()) {
                            //
                        }
                    }
                    Err(_) => {}
                }
            });
            reading.await;
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

#[cfg(test)]
mod tests {
    use crate::parse;

    #[test]
    fn test_parse() {
        let input = "POST /post-msg HTTP/1.1\nUser-Agent:curl/7.64.1\nHost:localhost:9443\nAccept:*/*\nContent-Length:52\nContent-Type:application/json\nForwarded:for=0:0:0:0:0:0:0:1;proto=https;host=localhost:9443;by=0:0:0:0:0:0:0:1\nX-Forwarded-For:0:0:0:0:0:0:0:1\nX-Forwarded-Proto:https\nX-Forwarded-Host:localhost:9443\nX-Forwarded-Server:0:0:0:0:0:0:0:1\n\n_1";
        parse(input)
    }

}
