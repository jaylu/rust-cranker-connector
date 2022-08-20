use mpsc::channel;
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use async_native_tls::TlsConnector;
use async_tungstenite::async_std::connect_async_with_tls_connector;
use async_tungstenite::tungstenite::handshake::client::generate_key;
use async_tungstenite::tungstenite::http::Request;
use async_tungstenite::tungstenite::http::request::Builder;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::tungstenite::protocol::CloseFrame;
use async_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use bytes::Bytes;
use futures::SinkExt;
use futures::StreamExt;
use http::Uri;
use hyper::{Body, Client, Method};
use hyper::body::{HttpBody, Sender};
use hyper::client::ResponseFuture;
use hyper::Request as hyper_request;

use uuid::Uuid;

mod lib;

fn startConnectorSocket(connector_instance_id: String,
                        registration_uri: String,
                        target_uri: String,
                        component_name: String,
                        sender: mpsc::Sender<SocketEvent>) {
    tokio::spawn(async move {
        ConnectorSocket::new(connector_instance_id, registration_uri, target_uri, component_name).start(sender).await;
    });
}

enum SocketEvent {
    CREATE(String),
    OPEN(String),
    CONSUMED(String),
    ERROR(String),
}

struct Config<> {
    route: String,
    target_uri: String,
    component_name: String,
    router_uri_provider: fn() -> Vec<String>,
    sliding_window: usize,
}

struct CrankerConnector {
    connector_instance_id: String,
    config: Config,
}

impl CrankerConnector {
    pub fn new(config: Config) -> CrankerConnector {
        return CrankerConnector {
            connector_instance_id: Uuid::new_v4().to_string(),
            config,
        };
    }

    pub async fn start(&mut self) -> &mut CrankerConnector {
        let registration_urls = (&self.config.router_uri_provider)();

        for registration_url in registration_urls {
            let mut registration = RouterRegistration::new(String::from(&self.connector_instance_id),
                                                           registration_url,
                                                           String::from(&self.config.target_uri),
                                                           String::from(&self.config.component_name),
                                                           self.config.sliding_window);
            registration.start();
        }


        // connect_to_router(target, &self.idle_count).await;
        return self;
    }

    pub fn stop(&mut self) -> &mut CrankerConnector {
        return self;
    }
}


struct RouterRegistration {
    connector_instance_id: String,
    registration_uri: String,
    target_uri: String,
    component_name: String,
    sliding_window: usize,
    idle_sockets: HashSet<String>,
}

impl RouterRegistration {
    pub fn new(connector_instance_id: String,
               registration_uri: String,
               target_uri: String,
               component_name: String,
               sliding_window: usize) -> RouterRegistration {
        RouterRegistration {
            connector_instance_id,
            registration_uri,
            target_uri,
            component_name,
            sliding_window,
            idle_sockets: HashSet::new(),
        }
    }

    pub async fn start(self) {
        let (sender, receiver): (mpsc::Sender<SocketEvent>, mpsc::Receiver<SocketEvent>) = channel();
        let mut idle_sockets = HashSet::new();


        let add_anything_missing = move |current_idle: &HashSet<String>, clone_sender: mpsc::Sender<SocketEvent>| {

            let connector_instance_id = String::from(&self.connector_instance_id);
            let registration_uri = String::from(&self.registration_uri);
            let target_uri = String::from(&self.target_uri);
            let component_name = String::from(&self.component_name);

            while current_idle.len() < self.sliding_window {
                let connector_instance_id_copy = String::from(&connector_instance_id);
                let registration_uri_copy = String::from(&registration_uri);
                let target_uri_copy = String::from(&target_uri);
                let component_name_copy = String::from(&component_name);
                let cc_sender = clone_sender.clone();
                tokio::spawn(async move {
                    ConnectorSocket::new(connector_instance_id_copy, registration_uri_copy,
                                         target_uri_copy, component_name_copy).start(cc_sender).await;
                });
            }
        };

        tokio::spawn(async move {
            for event in receiver.recv() {
                let sender_2 = sender.clone();
                match event {
                    SocketEvent::CREATE(socket_id) => {
                        idle_sockets.insert(socket_id);
                    },
                    SocketEvent::OPEN(socket_id )=> {

                    },
                    SocketEvent::CONSUMED(socket_id) => {
                        idle_sockets.remove(socket_id.as_str());
                        add_anything_missing(&idle_sockets, sender_2);
                    },
                    SocketEvent::ERROR(socket_id) => {
                        // TODO: sleep and debounce
                        idle_sockets.remove(socket_id.as_str());
                        add_anything_missing(&idle_sockets, sender_2);
                    }
                }
            }
        });

        // add_anything_missing(&idle_sockets);
    }

    pub fn add_anything_missing(connector_instance_id: String,
                                registration_uri: String,
                                target_uri: String,
                                component_name: String,
                                sender: mpsc::Sender<SocketEvent>) {

        tokio::spawn(async move {
            ConnectorSocket::new(connector_instance_id, registration_uri, target_uri, component_name).start(sender).await;
        });

        // while self.idle_sockets.len() < self.sliding_window as usize {
        //
        // }
    }
}

struct ConnectorSocket {
    connector_instance_id: String,
    registration_uri: String,
    target_uri: String,
    component_name: String,
    socket_id: String,
}

impl ConnectorSocket {
    pub fn new(connector_instance_id: String,
               registration_uri: String,
               target_uri: String,
               component_name: String) -> ConnectorSocket {
        ConnectorSocket {
            connector_instance_id,
            registration_uri,
            target_uri,
            component_name,
            socket_id: Uuid::new_v4().to_string(),
        }
    }

    async fn stop(&mut self) {}

    async fn start(&mut self, sender: mpsc::Sender<SocketEvent>) {
        self.connect_to_router(sender).await;
    }

    async fn connect_to_router(&self, sender: mpsc::Sender<SocketEvent>) {
        let client = Client::new();
        let connector = TlsConnector::new().danger_accept_invalid_certs(true);
        let uri = format!("{}/register?connectorInstanceID={}&componentName={}",
                          self.registration_uri, self.connector_instance_id, self.component_name);
        let target = &self.target_uri;
        let request = Request::builder()
            .method("GET")
            .header("Host", "localhost")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("CrankerProtocol", "1.0")
            .header("Route", "*")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_key())
            .uri(uri)
            .body(())
            .unwrap();

        match connect_async_with_tls_connector(request, Some(connector)).await {
            Ok((stream, _response)) => {
                let (mut write, mut read) = stream.split();

                let mut body_writer: Option<Sender> = None;
                let mut client_request_future: Option<ResponseFuture> = None;
                let mut is_consumed = false;

                while let Some(message) = read.next().await {
                    if !is_consumed {
                        sender.send(SocketEvent::CONSUMED(String::from(&self.socket_id))).unwrap();
                        is_consumed = true;
                    }

                    match message {
                        Ok(msg) => {
                            if msg.is_text() {
                                // construct http request
                                let text_line = msg.into_text().unwrap();
                                println!("text: {:?}", text_line);
                                if text_line.ends_with("_1") {

                                    // has body
                                    let (sender, body) = Body::channel();
                                    body_writer = Some(sender);

                                    let request_builder = lib::parse(&text_line, target);
                                    let req = request_builder.body(body).unwrap();

                                    client_request_future = Some(client.request(req));
                                } else if text_line.ends_with("_2") {

                                    // no body
                                    let req = lib::parse(&text_line, target).body(Body::empty()).unwrap();
                                    let mut resp = client.request(req).await.unwrap();

                                    match write.send(Message::Text(format!("HTTP/1.1 {} OK\n", resp.status()))).await {
                                        Ok(_) => { println!("tx response sent text") }
                                        Err(_) => { println!("tx response sent text error") }
                                    }

                                    while let Some(next) = resp.data().await {
                                        match next {
                                            Ok(chunk) => {
                                                match write.send(Message::Binary(chunk.to_vec())).await {
                                                    Ok(_) => { println!("tx response sent binary"); }
                                                    Err(_) => { println!("tx response sent binary error") }
                                                }
                                            }
                                            Err(err) => { println!("error {}", err) }
                                        }
                                    }

                                    match write.send(
                                        Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "normal close".into() }))
                                    ).await {
                                        Ok(_) => { println!("close done"); }
                                        Err(_) => { println!("close error") }
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
                                            Err(err) => {
                                                println!("error {}", err)
                                            }
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
                                        Ok(_) => { println!("writer send data done") }
                                        Err(_) => { println!("writer send data err") }
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
}

async fn async_fn() {
    println!("async_fn");
}

#[tokio::main]
async fn main() {
    // let config = Config {
    //     route: String::from("*"),
    //     target_uri: String::from("http://localhost:8080"),
    //     component_name: String::from("rust-testing-component"),
    //     router_uri_provider: || { vec![String::from("wss://localhost:16488/register?connectorId=abc")] },
    //     sliding_window: 2,
    // };
    // let mut connector = CrankerConnector::new(config);
    // connector.start().await;
    // test_main().await

}

fn test_async() {
    let mut handles = vec![];

    let handle1 = tokio::spawn(async {
        async_fn().await
    });
    handles.push(handle1);

    let handle2 = tokio::spawn(async {
        async_fn().await
    });
    handles.push(handle2);

    thread::sleep(Duration::from_secs(2));
}

#[cfg(test)]
mod tests {}
