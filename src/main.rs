use std::borrow::{Borrow, BorrowMut, Cow};
use std::str::FromStr;

use async_native_tls::TlsConnector;
use async_tungstenite::async_std::{connect_async, connect_async_with_config, connect_async_with_tls_connector, ConnectStream};
use async_tungstenite::tungstenite::{Error, Message};
use async_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use async_tungstenite::tungstenite::http::HeaderValue;
use async_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use async_tungstenite::tungstenite::http::request::Builder;
use async_tungstenite::tungstenite::protocol::CloseFrame;
use async_tungstenite::WebSocketStream;
use bytes::Bytes;
use futures::{executor, Sink, SinkExt, TryStreamExt};
use futures::stream::SplitSink;
use futures::StreamExt;
use hyper::{Body, Client, Method};
use hyper::body::HttpBody;
use hyper::Request as hyper_request;
use tokio::join;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

fn parse(input: &str) -> Builder {
    let lines: Vec<&str> = input.split("\n").collect();
    let first_line_fields: Vec<&str> = lines[0].split(" ").collect();
    let method = first_line_fields[0];
    let path = first_line_fields[1];
    // let version = first_line_fields[2];

    let mut request_builder = hyper_request::builder();
    for line in &lines[1..] {
        match line.find(":") {
            Some(index) => {
                request_builder = request_builder.header(&line[0..index], &line[index + 1..]);
            }
            None => ()
        }
    }

    return request_builder
        .method(Method::from_str(method).unwrap())
        .uri(format!("{}{}", "http://localhost:8080", path));
}

async fn connect_to_router() {
    let client = Client::new();
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
        Ok((mut stream, response)) => {
            let (mut write, read) = stream.split();

            // let (tx, mut rx): (Sender<Message>, Receiver<Message>) = mpsc::channel(32);

            let reading = read.for_each(|message| async {
                match message {
                    Ok(msg) => {
                        if msg.is_text() {
                            // construct http reqeust
                            let text_line = msg.into_text().unwrap();
                            println!("text: {:?}", text_line);
                            if text_line.ends_with("_1") {
                                // has body
                                println!("has_body");
                            } else if text_line.ends_with("_2") {
                                // no body
                                println!("no_body");

                                let request_builder = parse(&text_line);

                                let req = request_builder
                                    .body(Body::from(r#"{"library":"hyper"}"#)).unwrap();

                                let mut resp = client.request(req).await.unwrap();
                                println!("status={}", resp.status());

                                match &write.send(Message::Text(format!("HTTP/1.1 {} OK\n", resp.status()))).await {
                                    Ok(_) => {
                                        println!("tx sent text")
                                    }
                                    Err(_) => {
                                        println!("tx sent text error")
                                    }
                                }

                                while let Some(next) = resp.data().await {
                                    match next {
                                        Ok(chunk) => {
                                            // TODO: zero-copy
                                            // match write.send(Message::Binary(chunk.to_vec())).await {
                                            //     Ok(_) => { println!("tx sent binary") }
                                            //     Err(_) => { println!("tx sent binary error") }
                                            // }
                                        }
                                        Err(err) => {
                                            println!("error {}", err)
                                        }
                                    }
                                }

                                // done
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

            // let writing = do_send(&mut write, &mut rx);

            // reading.
        }
        Err(e) => {
            eprintln!("error: {:?}", e);
        }
    }
}

async fn do_send(write: &mut SplitSink<WebSocketStream<ConnectStream>, Message>, rx: &mut Receiver<Message>) {
    while let Some(message) = rx.recv().await {
        let send_result = write.send(message).await;
        match send_result {
            Ok(_) => { println!("write send") }
            Err(_) => { println!("write send error") }
        }
    }
    // TODO: fix
    println!("send done");
    match write.close().await {
        Ok(_) => {println!("close")}
        Err(_) => {println!("close error")}
    }
}

async fn test_main() {
    let client = Client::new();

    let req = hyper_request::builder()
        .method(Method::GET)
        .uri("http://localhost:8080/apply/")
        .body(Body::from(r#"{"library":"hyper"}"#)).unwrap();

    let mut resp = client.request(req).await.unwrap();
    println!("status={}", resp.status());


    while let Some(next) = resp.data().await {
        let chunk = next.unwrap();
        let body_string = String::from_utf8(chunk.to_vec()).unwrap();
        print!("{}", body_string)
    }


    // let bytes = hyper::body::to_bytes(resp.into_body()).await.unwrap();
    // let body_string = String::from_utf8(bytes.to_vec()).unwrap();
    // println!("body={}", body_string);
}

#[tokio::main]
async fn main() {
    connect_to_router().await
    // test_main().await
}

#[cfg(test)]
mod tests {
    use crate::parse;

    #[test]
    fn test_parse() {
        let input = "POST /post-msg HTTP/1.1\nUser-Agent:curl/7.64.1\nHost:localhost:9443\nAccept:*/*\nContent-Length:52\nContent-Type:application/json\nForwarded:for=0:0:0:0:0:0:0:1;proto=https;host=localhost:9443;by=0:0:0:0:0:0:0:1\nX-Forwarded-For:0:0:0:0:0:0:0:1\nX-Forwarded-Proto:https\nX-Forwarded-Host:localhost:9443\nX-Forwarded-Server:0:0:0:0:0:0:0:1\n\n_1";
        let builder = parse(input);
        let headers = builder.headers_ref().unwrap();
        assert_eq!(headers["X-Forwarded-Server"], "0:0:0:0:0:0:0:1");
        assert_eq!(headers["Accept"], "*/*");
    }

    #[test]
    fn test_vec() {
        let sample = vec!["a", "b", "c"];
        let index = 2;
        println!("output: {:?}", &sample[0..index]);
    }
}
