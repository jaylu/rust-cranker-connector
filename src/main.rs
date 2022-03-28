use std::str::FromStr;

use async_native_tls::TlsConnector;
use async_tungstenite::async_std::{
    connect_async, connect_async_with_config, connect_async_with_tls_connector,
};
use async_tungstenite::tungstenite::{Error, Message};
use async_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use async_tungstenite::tungstenite::http::HeaderValue;
use async_tungstenite::tungstenite::http::request::Builder;
use futures::{executor, TryStreamExt};
use futures::StreamExt;
use hyper::{Body, Client, Method};
use hyper::body::HttpBody;
use hyper::Request as hyper_request;

async fn parse(input: &'static str) -> Builder {
    let lines: Vec<&str> = input.split("\n").collect();
    let first_line_fields: Vec<&str> = lines[0].split(" ").collect();
    let method = first_line_fields[0];
    let path = first_line_fields[1];
    // let version = first_line_fields[2];

    let mut request_builder = hyper_request::builder();
    let headers = request_builder.headers_mut().unwrap();
    for line in &lines[1..] {
        match line.find(":") {
            Some(index) => {
                headers.insert(&line[0..index], HeaderValue::from_static(&line[index..]));
            },
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
    // connect_to_router().await
    test_main().await
}

#[cfg(test)]
mod tests {
    use futures::executor;

    use crate::parse;

    #[test]
    fn test_parse() {
        let input = "POST /post-msg HTTP/1.1\nUser-Agent:curl/7.64.1\nHost:localhost:9443\nAccept:*/*\nContent-Length:52\nContent-Type:application/json\nForwarded:for=0:0:0:0:0:0:0:1;proto=https;host=localhost:9443;by=0:0:0:0:0:0:0:1\nX-Forwarded-For:0:0:0:0:0:0:0:1\nX-Forwarded-Proto:https\nX-Forwarded-Host:localhost:9443\nX-Forwarded-Server:0:0:0:0:0:0:0:1\n\n_1";
        executor::block_on(parse(input));
    }

    #[test]
    fn test_vec() {
        let sample = vec!["a", "b", "c"];
        let index = 2;
        println!("output: {:?}", &sample[0..index]);
    }
}
