use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use async_native_tls::TlsConnector;
use async_tungstenite::async_std::connect_async_with_tls_connector;
use async_tungstenite::tungstenite::handshake::client::generate_key;
use async_tungstenite::tungstenite::http::Request;
use async_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use async_tungstenite::tungstenite::protocol::CloseFrame;
use async_tungstenite::tungstenite::{Error, Message};
use bytes::Bytes;
use dashmap::DashSet;
use futures::{future, StreamExt};
use futures::{FutureExt, SinkExt};
use http::Response;
use hyper::body::{HttpBody};
use hyper::client::{HttpConnector, ResponseFuture};
use hyper::{Body, Client};

use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio::time;
use tokio::time::Instant;

use uuid::Uuid;

mod lib;

enum RegistrationEvent {
    RegistrationInit,
    SocketCreate(String),
    SocketOpen(String),
    SocketConsumed(String),
    SocketError(String),
}

struct Config {
    route: String,
    target_uri: String,
    component_name: String,
    router_uri_provider: fn() -> Vec<String>,
    sliding_window: usize,
    tls_connector_provider: fn() -> Option<TlsConnector>,
}

struct CrankerConnector {
    connector_instance_id: String,
    config: Config,
    client: Client<HttpConnector>,
}

impl CrankerConnector {
    pub fn new(config: Config) -> CrankerConnector {
        return CrankerConnector {
            connector_instance_id: Uuid::new_v4().to_string(),
            config,
            client: Client::new(),
        };
    }

    pub async fn start(&mut self) -> &mut CrankerConnector {
        let registration_urls = (&self.config.router_uri_provider)();

        for registration_url in registration_urls {
            let registration = RouterRegistration::new(
                self.client.clone(),
                self.config.tls_connector_provider,
                String::from(&self.connector_instance_id),
                registration_url,
                String::from(&self.config.target_uri),
                String::from(&self.config.component_name),
                String::from(&self.config.route),
                self.config.sliding_window,
            );
            tokio::spawn(async {
                registration.start().await;
            });
        }
        return self;
    }

    pub fn stop(&mut self) -> &mut CrankerConnector {
        return self;
    }
}

struct RouterRegistration {
    client: Client<HttpConnector>,
    tls_connector_provider: fn() -> Option<TlsConnector>,
    connector_instance_id: String,
    registration_uri: String,
    target_uri: String,
    component_name: String,
    route: String,
    sliding_window: usize,
}

impl RouterRegistration {
    pub fn new(
        client: Client<HttpConnector>,
        tls_connector_provider: fn() -> Option<TlsConnector>,
        connector_instance_id: String,
        registration_uri: String,
        target_uri: String,
        component_name: String,
        route: String,
        sliding_window: usize,
    ) -> RouterRegistration {
        RouterRegistration {
            tls_connector_provider,
            client,
            connector_instance_id,
            registration_uri,
            target_uri,
            component_name,
            route,
            sliding_window,
        }
    }

    pub async fn start(self) {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1024);
        let sender_2 = sender.clone();
        let sender_3 = sender.clone();
        let idle_sockets = Arc::new(DashSet::new());

        let idle_sockets_2 = Arc::clone(&idle_sockets);
        let add_anything_missing = move || {
            println!(
                "add anything missing, idles_sockets size={}",
                idle_sockets_2.len()
            );

            while idle_sockets_2.len() < self.sliding_window {
                println!("debug: creating connector socket");
                let mut socket = ConnectorSocket::new(
                    self.client.clone(),
                    self.tls_connector_provider,
                    String::from(&self.connector_instance_id),
                    String::from(&self.registration_uri),
                    String::from(&self.target_uri),
                    String::from(&self.component_name),
                    String::from(&self.route),
                    sender_3.clone(),
                );
                let socket_id = String::from(&socket.socket_id);
                let socket_id_clone = socket_id.clone();
                idle_sockets_2.insert(socket_id);
                println!(
                    "debug: idles_sockets add {}, size={}",
                    socket_id_clone,
                    idle_sockets_2.len()
                );

                tokio::spawn(async move {
                    println!("debug: starting connector socket");
                    socket.start().await;
                });
            }
        };

        tokio::spawn(async move {
            println!("debug: start receiving event");
            let sender_3 = sender.clone();
            while let Some(event) = receiver.recv().await {
                match event {
                    RegistrationEvent::RegistrationInit => {
                        add_anything_missing();
                    }
                    RegistrationEvent::SocketCreate(socket_id) => {
                        let string = String::from(&socket_id);
                        idle_sockets.insert(string);
                        println!(
                            "debug: idles_sockets add {}, size={}",
                            &socket_id,
                            idle_sockets.len()
                        );
                    }
                    RegistrationEvent::SocketOpen(_socket_id) => {}
                    RegistrationEvent::SocketConsumed(socket_id) => {
                        idle_sockets.remove(&socket_id);
                        // println!("debug: idles_sockets remove {}, size={}", &socket_id, idle_sockets.len());
                        add_anything_missing();
                    }
                    RegistrationEvent::SocketError(socket_id) => {
                        // TODO: sleep and debounce
                        idle_sockets.remove(&socket_id);
                        // println!("debug: idles_sockets remove {}, size={}", &socket_id, idle_sockets.len());
                        add_anything_missing();
                    }
                }
            }
            println!("debug: sender state {:?}", sender_3);
            println!("debug: stop receiving event");
        });

        match sender_2.send(RegistrationEvent::RegistrationInit).await {
            Ok(x) => x,
            Err(_) => todo!(),
        };
        println!("route registration started");
    }
}

struct ConnectorSocket {
    client: Client<HttpConnector>,
    tls_connector_provider: fn() -> Option<TlsConnector>,
    connector_instance_id: String,
    registration_uri: String,
    target_uri: String,
    component_name: String,
    route: String,
    socket_id: String,
    registration_sender: tokio::sync::mpsc::Sender<RegistrationEvent>,
}

impl ConnectorSocket {
    pub fn new(
        client: Client<HttpConnector>,
        tls_connector_provider: fn() -> Option<TlsConnector>,
        connector_instance_id: String,
        registration_uri: String,
        target_uri: String,
        component_name: String,
        route: String,
        sender: tokio::sync::mpsc::Sender<RegistrationEvent>,
    ) -> ConnectorSocket {
        ConnectorSocket {
            client,
            tls_connector_provider,
            connector_instance_id,
            registration_uri,
            target_uri,
            component_name,
            route,
            socket_id: Uuid::new_v4().to_string(),
            registration_sender: sender,
            // internal_sender: None,
        }
    }

    async fn stop(&mut self) {
        // if self.internal_sender.is_some() {
        //     println!("stopping connector socket");
        //     match self
        //         .internal_sender
        //         .clone()
        //         .unwrap()
        //         .send(SocketEvent::Stop)
        //         .await
        //     {
        //         Ok(_) => {}
        //         Err(_) => {}
        //     };
        // }
    }

    async fn start(&mut self) {
        self.connect_to_router(self.client.clone(), self.tls_connector_provider).await;
    }

    async fn connect_to_router(
        &mut self,
        client: Client<HttpConnector>,
        tls_connector_provider: fn() -> Option<TlsConnector>,
    ) {
        let sender = &self.registration_sender.clone();
        let connector = tls_connector_provider().unwrap();
        let uri = format!( "{}/register?connectorInstanceID={}&componentName={}",
            self.registration_uri, self.connector_instance_id, self.component_name
        );
        let target = &self.target_uri;
        let route = &self.route;
        let socket_id = String::from(&self.socket_id);
        let request = Request::builder()
            .method("GET")
            .header("Host", "localhost")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("CrankerProtocol", "1.0")
            .header("Route", route)
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_key())
            .uri(&uri)
            .body(())
            .unwrap();

        let is_started = Arc::new(AtomicBool::new(false));
        let is_started_clone = Arc::clone(&is_started);

        println!("connector socket: connecting to {}", &uri);

        match connect_async_with_tls_connector(request, Some(connector)).await {
            Ok((stream, _response)) => {
                let (mut wss_write, mut read) = stream.split();

                let mut target_body_writer: Option<hyper::body::Sender> = None;
                let mut target_request_future: Option<ResponseFuture> = None;
                let mut is_consumed = false;

                let (write, mut wss_bridge_rx) = tokio::sync::mpsc::channel(10);

                tokio::spawn(async move {
                    let mut ping_interval = time::interval(Duration::from_secs(5));
                    loop {
                        tokio::select! {
                            Some(event) = wss_bridge_rx.recv()  => {
                                println!("received event: {}", event);
                                 match wss_write.send(event).await {
                                    Ok(_) => {println!("ws send Ok");}
                                    Err(_) => {println!("ws send error")}
                                }
                            }
                            _ = ping_interval.tick() => {
                                println!("tokio select: sending heart beat {}", socket_id);
                                match wss_write.send(Message::Ping("ping".as_bytes().to_vec())).await{
                                    Ok(_) => { println!("ping sent")}
                                    Err(_) => {println!("ping sent error")}
                                }
                            }
                        }
                    }
                });

                while let Some(message) = read.next().await {
                    println!("message loop");
                    match message {
                        Ok(Message::Text(message)) => {
                            if !is_consumed {
                                match sender.send(RegistrationEvent::SocketConsumed(String::from(&self.socket_id))).await {
                                    Ok(()) => { println!("ok"); }
                                    Err(error) => { println!("error: {}", error); }
                                }
                                is_consumed = true;
                            }

                            // construct http request
                            let text_line = message;
                            println!("text: {:?}", text_line);

                            if text_line.ends_with("_1") {
                                // has body

                                let (sender, body) = Body::channel();
                                target_body_writer = Some(sender);

                                let req = lib::parse(&text_line, target).body(body).unwrap();
                                target_request_future = Some(client.request(req));

                            } else if text_line.ends_with("_2") {
                                // no body

                                let req = lib::parse(&text_line, target)
                                    .body(Body::empty())
                                    .unwrap();

                                let mut response = client.request(req).await.unwrap();

                                match write.send(Message::Text(format!("HTTP/1.1 {} OK\n", response.status().as_u16()))).await {
                                    Ok(_) => { println!("sent done"); }
                                    Err(_) => { println!("sent error"); }
                                };

                                Self::handle_response(&write, &mut response).await;
                            } else if text_line.ends_with("_3") {
                                // body send completed

                                let mut response = target_request_future.as_mut().unwrap().await.unwrap();
                                match write.send(Message::Text(format!("HTTP/1.1 {} OK\n", response.status().as_u16()))).await {
                                    Ok(_) => { println!("sent done"); }
                                    Err(_) => { println!("sent error"); }
                                };
                                Self::handle_response(&write, &mut response).await;
                            } else {}
                        }
                        Ok(Message::Binary(message)) => {
                            println!("receiving byte body");
                            if let Some(target_body_writer) = target_body_writer.as_mut() {
                                match target_body_writer.send_data(Bytes::from(message)).await {
                                    Ok(_) => { println!("writer send data done"); }
                                    Err(_) => { println!("writer send data err"); }
                                };
                            }
                        }
                        Ok(Message::Ping(_)) => {}
                        Ok(Message::Pong(_)) => {}
                        Ok(Message::Close(_)) => {}
                        Ok(Message::Frame(_)) => {}
                        Err(_) => {
                            println!("tokio select: ws message error");
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("error: {:?}", e);
            }
        }
    }

    async fn handle_response(write: &Sender<Message>, response: &mut Response<Body>) {
        while let Some(next) = response.data().await {
            match next {
                Ok(chunk) => {
                    println!("handle_response: sending bytes");
                    match write.send(Message::Binary(chunk.to_vec())).await {
                        Ok(_) => { println!("sent done"); }
                        Err(_) => { println!("sent error"); }
                    };
                }
                Err(err) => { println!("error {}", err); }
            }
        };

        println!("handle_response: closing");
        match write.send(Message::Close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: "normal close".into(),
        }))).await {
            Ok(_) => { println!("sent done"); }
            Err(_) => { println!("sent error"); }
        };

        println!("write close");
    }
}

#[tokio::main]
async fn main() {
    let config = Config {
        route: String::from("*"),
        target_uri: String::from("http://localhost:8080"),
        component_name: String::from("rust-testing-component"),
        router_uri_provider: || vec![String::from("wss://localhost:12001")],
        sliding_window: 1,
        tls_connector_provider: || Some(TlsConnector::new().danger_accept_invalid_certs(true)),
    };
    let mut connector = CrankerConnector::new(config);
    connector.start().await;

    thread::sleep(Duration::from_secs(3000))
}

#[cfg(test)]
mod tests {}
