use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::thread;
use std::time::Duration;

use async_native_tls::TlsConnector;
use async_tungstenite::async_std::connect_async_with_tls_connector;
use async_tungstenite::tungstenite::handshake::client::generate_key;
use async_tungstenite::tungstenite::http::Request;
use async_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use async_tungstenite::tungstenite::protocol::CloseFrame;
use async_tungstenite::tungstenite::Message;
use bytes::Bytes;
use dashmap::DashSet;
use futures::SinkExt;
use futures::StreamExt;
use http::Response;
use hyper::body::HttpBody;
use hyper::client::{HttpConnector, ResponseFuture};
use hyper::{Body, Client};

use tokio::sync::broadcast;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time;

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
            let mut registration = RouterRegistration::new(
                self.client.clone(),
                self.config.tls_connector_provider,
                String::from(&self.connector_instance_id),
                registration_url,
                String::from(&self.config.target_uri),
                String::from(&self.config.component_name),
                String::from(&self.config.route),
                self.config.sliding_window,
            );
            tokio::spawn(async move {
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
    idle_sockets: Arc<DashSet<String>>,
    event_tx: Sender<RegistrationEvent>,
    event_rx: Receiver<RegistrationEvent>,
    failureAttempts: Arc<AtomicU32>
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
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(1024);
        RouterRegistration {
            tls_connector_provider,
            client,
            connector_instance_id,
            registration_uri,
            target_uri,
            component_name,
            route,
            sliding_window,
            idle_sockets: Arc::new(DashSet::new()),
            event_tx,
            event_rx,
            failureAttempts: Arc::new(AtomicU32::new(0))
        }
    }

    fn start_socket(&mut self) {
        let mut socket = ConnectorSocket::new(
            self.client.clone(),
            self.tls_connector_provider,
            String::from(&self.connector_instance_id),
            String::from(&self.registration_uri),
            String::from(&self.target_uri),
            String::from(&self.component_name),
            String::from(&self.route),
            self.event_tx.clone(),
        );
        let socket_id = String::from(&socket.socket_id);
        let _ = &self.idle_sockets.insert(socket_id);

        tokio::spawn(async move {
            socket.start().await;
        });
    }

    async fn add_anything_missing(&mut self) {
        // println!("add_anything_missing");
        while self.idle_sockets.len() < self.sliding_window {
            self.start_socket();
        }
    }

    pub async fn start(&mut self) {
        self.add_anything_missing().await;
        println!("route registration started");
        while let Some(event) = self.event_rx.recv().await {
            match event {
                RegistrationEvent::RegistrationInit => {
                    self.add_anything_missing().await;
                }
                RegistrationEvent::SocketCreate(socket_id) => {
                    let string = String::from(&socket_id);
                    self.idle_sockets.insert(string);
                }
                RegistrationEvent::SocketOpen(_socket_id) => {
                    self.failureAttempts.store(0, std::sync::atomic::Ordering::Relaxed)
                }
                RegistrationEvent::SocketConsumed(socket_id) => {
                    self.idle_sockets.remove(&socket_id);
                    self.add_anything_missing().await;
                }
                RegistrationEvent::SocketError(socket_id) => {
                    // TODO: sleep and debounce
                    self.failureAttempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.idle_sockets.remove(&socket_id);
                    let attempts = self.failureAttempts.load(std::sync::atomic::Ordering::Relaxed);
                    let back_out_time = Duration::from_millis(lib::back_out(attempts));
                    println!("going to add_anything_missing with back out time {:?}", back_out_time);
                    tokio::time::sleep(back_out_time).await;
                    self.add_anything_missing().await;
                }
            }
        }
        println!("route registration stopped");
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
    registration_sender: Sender<RegistrationEvent>,
    notify_stop: broadcast::Sender<()>,
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
        sender: Sender<RegistrationEvent>,
    ) -> ConnectorSocket {
        let (notify_stop, _) = broadcast::channel(1);

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
            notify_stop,
        }
    }

    fn stop(&mut self) {
        // TODO: skip if stopped
        let _ = self.notify_stop.send(());
    }

    async fn start(&mut self) {
        self.connect_to_router(self.client.clone(), self.tls_connector_provider).await;
        println!("socket completed {}", self.socket_id);
    }

    async fn connect_to_router(&mut self, client: Client<HttpConnector>, tls_connector_provider: fn() -> Option<TlsConnector>) {
        let registration_sender = &self.registration_sender.clone();
        let connector = tls_connector_provider().unwrap();
        let uri = format!(
            "{}/register?connectorInstanceID={}&componentName={}",
            self.registration_uri, self.connector_instance_id, self.component_name
        );
        let target = String::from(&self.target_uri);
        let route = String::from(&self.route);
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

        match connect_async_with_tls_connector(request, Some(connector)).await {
            Ok((stream, _response)) => {
                let (mut wss_write, mut read) = stream.split();

                let mut target_body_writer: Option<hyper::body::Sender> = None;
                let mut target_request_future: Option<ResponseFuture> = None;
                let mut is_consumed = false;

                let (write, mut wss_channel_rx) = tokio::sync::mpsc::channel(1);

                let notify_stop = self.notify_stop.clone();
                tokio::spawn(async move {
                    let mut ping_interval = time::interval(Duration::from_secs(5));
                    let mut notify_stop = notify_stop.subscribe();
                    loop {
                        tokio::select! {
                            Some(event) = wss_channel_rx.recv()  => {
                                // println!("received event: {}", event);
                                 match wss_write.send(event).await {
                                    Ok(_) => {
                                        // println!("ws send Ok");
                                    }
                                    Err(_) => {println!("ws send error")}
                                }
                            }
                            _ = ping_interval.tick() => {
                                println!("tokio select: sending heart beat {}", socket_id);
                                match wss_write.send(Message::Ping("ping".as_bytes().to_vec())).await{
                                    Ok(_) => { 
                                        // println!("ping sent")
                                    }
                                    Err(_) => {println!("ping sent error")}
                                }
                            }
                            _ = notify_stop.recv() => {
                                break;
                            }
                        }
                    }
                    println!("connector socket stop _1 {}", socket_id)
                });

                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(message)) => {
                            if !is_consumed {
                                match registration_sender.send(RegistrationEvent::SocketConsumed(String::from(&self.socket_id))).await {
                                    Ok(()) => {
                                        // println!("ok");
                                    }
                                    Err(error) => {
                                        println!("error: {}", error);
                                    }
                                }
                                is_consumed = true;
                            }

                            // construct http request
                            let text_line = message;
                        
                            if text_line.ends_with("_1") {
                                // has body

                                let (sender, body) = Body::channel();
                                target_body_writer = Some(sender);

                                let req = lib::parse(&text_line, target.as_str()).body(body).unwrap();
                                target_request_future = Some(client.request(req));
                            } else if text_line.ends_with("_2") {
                                // no body

                                let req = lib::parse(&text_line, target.as_str()).body(Body::empty()).unwrap();

                                let mut response = client.request(req).await.unwrap();
                                let headers = response.headers().into_iter().fold(String::new(), |mut acc, (key, value)| {
                                    acc.push_str(format!("{}:{}\n", key.to_string().as_str(), value.to_str().unwrap().to_string().as_str()).as_str());
                                    acc
                                });

                                match write.send(Message::Text(format!("HTTP/1.1 {} OK\n{}", response.status().as_u16(), headers))).await {
                                    Ok(_) => {
                                        // println!("sent done");
                                    }
                                    Err(_) => {
                                        println!("sent error");
                                    }
                                };

                                Self::handle_response(&write, &mut response).await;
                            } else if text_line.ends_with("_3") {
                                // body send completed

                                let mut response = target_request_future.as_mut().unwrap().await.unwrap();
                                let headers = response.headers().into_iter().fold(String::new(), |mut acc, (key, value)| {
                                    acc.push_str(format!("{}:{}\n", key.to_string().as_str(), value.to_str().unwrap().to_string().as_str()).as_str());
                                    acc
                                });

                                match write.send(Message::Text(format!("HTTP/1.1 {} OK\n{}", response.status().as_u16(), headers))).await {
                                    Ok(_) => {
                                        // println!("sent done");
                                    }
                                    Err(_) => {
                                        println!("sent error");
                                    }
                                };
                                Self::handle_response(&write, &mut response).await;
                            } else {
                            }
                        }
                        Ok(Message::Binary(message)) => {
                            // println!("receiving byte body");
                            if let Some(target_body_writer) = target_body_writer.as_mut() {
                                match target_body_writer.send_data(Bytes::from(message)).await {
                                    Ok(_) => {
                                        // println!("writer send data done");
                                    }
                                    Err(_) => {
                                        println!("writer send data err");
                                    }
                                };
                            }
                        }
                        Ok(Message::Ping(_)) => {}
                        Ok(Message::Pong(_)) => {}
                        Ok(Message::Close(_)) => {}
                        Ok(Message::Frame(_)) => {}
                        Err(_) => {
                            self.stop();
                            let _ = registration_sender.send(RegistrationEvent::SocketError(String::from(&self.socket_id))).await;
                        }
                    }
                }

                self.stop();
            }
            Err(e) => {
                eprintln!("error: {:?}", e);
            }
        }

        println!("connector socket stop _2 {}", &self.socket_id);
    }

    async fn handle_response(write: &Sender<Message>, response: &mut Response<Body>) {
        while let Some(next) = response.data().await {
            match next {
                Ok(data) => {
                    let mut chunks  = data.chunks(65536);
                    while let Some(chunk) = chunks.next() {
                        println!("handle_response: sending bytes {}", chunk.len());
                        match write.send(Message::Binary(chunk.to_vec())).await {
                            Ok(_) => {
                                // println!("sent done");
                            }
                            Err(_) => {
                                println!("sent error");
                            }
                        };
                    }
                }
                Err(err) => {
                    println!("error {}", err);
                }
            }
        }

        println!("handle_response: closing");
        match write
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "normal close".into(),
            })))
            .await
        {
            Ok(_) => {
                println!("sent done");
            }
            Err(_) => {
                println!("sent error");
            }
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
        sliding_window: 2,
        tls_connector_provider: || Some(TlsConnector::new().danger_accept_invalid_certs(true)),
    };

    let mut connector = CrankerConnector::new(config);
    connector.start().await;

    thread::sleep(Duration::from_secs(3000))
}

#[cfg(test)]
mod tests {}
