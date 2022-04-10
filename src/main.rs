use std::str::FromStr;

use async_native_tls::TlsConnector;
use async_tungstenite::async_std::{connect_async_with_tls_connector};
use async_tungstenite::tungstenite::handshake::client::generate_key;
use async_tungstenite::tungstenite::http::Request;
use async_tungstenite::tungstenite::http::request::Builder;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::tungstenite::protocol::CloseFrame;
use async_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use bytes::Bytes;
use futures::{SinkExt};
use futures::StreamExt;
use hyper::{Body, Client, Method};
use hyper::body::{HttpBody, Sender};
use hyper::client::{ResponseFuture};
use hyper::Request as hyper_request;

fn parse(input: &str) -> Builder {

    let lines: Vec<&str> = input.split("\n").collect();
    let first_line_fields: Vec<&str> = lines[0].split(" ").collect();
    let method = first_line_fields[0];
    let path = first_line_fields[1];

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
        Ok((stream, _response)) => {
            let (mut write, mut read) = stream.split();

            let mut body_writer: Option<Sender> = None;
            let mut client_request_future: Option<ResponseFuture> = None;

            while let Some(message) = read.next().await {
                match message {
                    Ok(msg) => {
                        if msg.is_text() {
                            // construct http reqeust
                            let text_line = msg.into_text().unwrap();
                            println!("text: {:?}", text_line);
                            if text_line.ends_with("_1") {

                                // has body

                                let (sender , body) = Body::channel();
                                body_writer = Some(sender);

                                let request_builder = parse(&text_line);
                                let req = request_builder.body(body).unwrap();

                                client_request_future = Some(client.request(req));

                            } else if text_line.ends_with("_2") {

                                // no body

                                let req = parse(&text_line).body(Body::empty()).unwrap();
                                let mut resp = client.request(req).await.unwrap();

                                match write.send(Message::Text(format!("HTTP/1.1 {} OK\n", resp.status()))).await {
                                    Ok(_) => { println!("tx sent text") }
                                    Err(_) => { println!("tx sent text error") }
                                }

                                while let Some(next) = resp.data().await {
                                    match next {
                                        Ok(chunk) => {
                                            match write.send(Message::Binary(chunk.to_vec())).await {
                                                Ok(_) => { println!("tx sent binary"); }
                                                Err(_) => { println!("tx sent binary error") }
                                            }
                                        }
                                        Err(err) => {println!("error {}", err) }
                                    }
                                }

                                match write.send(
                                    Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "normal close".into() }))
                                ).await {
                                    Ok(_) => { println!("close done"); }
                                    Err(_) => { println!("tx sent binary error") }
                                }

                                println!("write close")

                            } else if text_line.ends_with("_3") {

                                // body send completed

                                let mut response = client_request_future.as_mut().unwrap().await.unwrap();

                                match write.send(Message::Text(format!("HTTP/1.1 {} OK\n", response.status()))).await {
                                    Ok(_) => { println!("tx sent text") }
                                    Err(_) => { println!("tx sent text error") }
                                }

                                // write.send_all(resp.body().data().into_stream());
                                while let Some(next) = response.data().await {
                                    match next {
                                        Ok(chunk) => {
                                            match write.send(Message::Binary(chunk.to_vec())).await {
                                                Ok(_) => { println!("tx sent binary"); }
                                                Err(_) => { println!("tx sent binary error") }
                                            }
                                        }
                                        Err(err) => {println!("error {}", err) }
                                    }
                                }

                                match write.send(
                                    Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "normal close".into() }))
                                ).await {
                                    Ok(_) => { println!("close done"); }
                                    Err(_) => { println!("tx sent binary error") }
                                }

                                println!("write close")

                            } else {}
                        } else if msg.is_binary() {

                            if body_writer.as_mut().is_some() {
                                let writer = body_writer.as_mut().unwrap();
                                match writer.send_data(Bytes::from(msg.into_data())).await {
                                    Ok(_) => {println!("writer send data done")},
                                    Err(_) => {println!("writer send data err")},
                                };
                            }

                        } else if msg.is_close() {
                            // clean up
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        Err(e) => {
            eprintln!("error: {:?}", e);
        }
    }
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
