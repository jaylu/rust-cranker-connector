use mpsc::channel;
use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;


use async_native_tls::TlsConnector;
use async_tungstenite::async_std::connect_async_with_tls_connector;
use async_tungstenite::tungstenite::handshake::client::generate_key;
use async_tungstenite::tungstenite::http::Request;
use async_tungstenite::tungstenite::{Error, Message};
use async_tungstenite::tungstenite::protocol::CloseFrame;
use async_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use bytes::Bytes;
use dashmap::DashSet;
use futures::SinkExt;
use futures::StreamExt;
use hyper::{Body, Client};
use hyper::body::{HttpBody, Sender};
use hyper::client::ResponseFuture;
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
            let registration = RouterRegistration::new(String::from(&self.connector_instance_id),
                                                       registration_url,
                                                       String::from(&self.config.target_uri),
                                                       String::from(&self.config.component_name),
                                                       String::from(&self.config.route),
                                                       self.config.sliding_window);
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
    connector_instance_id: String,
    registration_uri: String,
    target_uri: String,
    component_name: String,
    route: String,
    sliding_window: usize,
}

impl RouterRegistration {
    pub fn new(connector_instance_id: String,
               registration_uri: String,
               target_uri: String,
               component_name: String,
               route: String,
               sliding_window: usize) -> RouterRegistration {
        RouterRegistration {
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
        let idle_sockets = Arc::new(DashSet::new());
        let idle_sockets_2 = Arc::clone(&idle_sockets);

        let add_anything_missing = move |clone_sender: tokio::sync::mpsc::Sender<RegistrationEvent>| {
            println!("add anything missing, idles_sockets size={}", idle_sockets_2.len());

            while idle_sockets_2.len() < self.sliding_window {
                let connector_instance_id = String::from(&self.connector_instance_id);
                let registration_uri = String::from(&self.registration_uri);
                let target_uri = String::from(&self.target_uri);
                let component_name = String::from(&self.component_name);
                let route = String::from(&self.route);
                let cc_sender = clone_sender.clone();
                println!("debug: creating connector socket");

                let mut socket = ConnectorSocket::new(connector_instance_id, registration_uri, target_uri, component_name, route);
                let socket_id = String::from(&socket.socket_id);
                let socket_id_clone = socket_id.clone();
                idle_sockets_2.insert(socket_id);
                println!("debug: idles_sockets add {}, size={}", socket_id_clone, idle_sockets_2.len());

                tokio::spawn(async move {
                    println!("debug: starting connector socket");
                    socket.start(cc_sender).await;
                });
            }
        };

        tokio::spawn(async move {
            println!("debug: start receiving event");
            let sender_3 = sender.clone();
            while let Some(event) = receiver.recv().await {
                println!("debug: received event");
                match event {
                    RegistrationEvent::RegistrationInit => {
                        add_anything_missing(sender_3.clone());
                    }
                    RegistrationEvent::SocketCreate(socket_id) => {
                        let string = String::from(&socket_id);
                        idle_sockets.insert(string);
                        println!("debug: idles_sockets add {}, size={}", &socket_id, idle_sockets.len());
                    }
                    RegistrationEvent::SocketOpen(_socket_id) => {}
                    RegistrationEvent::SocketConsumed(socket_id) => {
                        idle_sockets.remove(&socket_id);
                        // println!("debug: idles_sockets remove {}, size={}", &socket_id, idle_sockets.len());
                        add_anything_missing(sender_3.clone());
                    }
                    RegistrationEvent::SocketError(socket_id) => {
                        // TODO: sleep and debounce
                        idle_sockets.remove(&socket_id);
                        // println!("debug: idles_sockets remove {}, size={}", &socket_id, idle_sockets.len());
                        add_anything_missing(sender_3.clone());
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
    connector_instance_id: String,
    registration_uri: String,
    target_uri: String,
    component_name: String,
    route: String,
    socket_id: String,

}

impl ConnectorSocket {
    pub fn new(connector_instance_id: String,
               registration_uri: String,
               target_uri: String,
               component_name: String,
               route: String) -> ConnectorSocket {
        ConnectorSocket {
            connector_instance_id,
            registration_uri,
            target_uri,
            component_name,
            route,
            socket_id: Uuid::new_v4().to_string(),
        }
    }

    async fn stop(&mut self) {

    }


    async fn start(&mut self, sender: tokio::sync::mpsc::Sender<RegistrationEvent>) {
        self.connect_to_router(sender).await;
    }

    async fn connect_to_router(&self, sender: tokio::sync::mpsc::Sender<RegistrationEvent>) {
        let client = Client::new();
        let connector = TlsConnector::new().danger_accept_invalid_certs(true);
        let uri = format!("{}/register?connectorInstanceID={}&componentName={}",
                          self.registration_uri,
                          self.connector_instance_id,
                          self.component_name);
        let target = &self.target_uri;
        let route = &self.route;
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

        let mut isStarted = AtomicBool::new(false);

        println!("connector socket: connecting to {}", &uri);

        match connect_async_with_tls_connector(request, Some(connector)).await {
            Ok((stream, _response)) => {
                let (mut write, mut read) = stream.split();

                let mut body_writer: Option<Sender> = None;
                let mut client_request_future: Option<ResponseFuture> = None;
                let mut is_consumed = false;

                // setup ping pong heart beat
                *isStarted.get_mut() = false;
                let mut ping_interval = time::interval(Duration::from_secs(5));
                while *isStarted.get_mut() {
                    ping_interval.tick().await;
                    match write.send(Message::Ping("ping".as_bytes().to_vec())).await {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }

                &ping_interval.tick().await;

                while let Some(message) = read.next().await {
                    match message {
                        Ok(msg) => {
                            match msg {
                                Message::Text(_) => {
                                    // construct http request
                                    let text_line = msg.into_text().unwrap();
                                    println!("text: {:?}", text_line);

                                    if !is_consumed {
                                        match sender.send(RegistrationEvent::SocketConsumed(String::from(&self.socket_id))).await {
                                            Ok(()) => {}
                                            Err(error) => println!("error: {}", error)
                                        }
                                        is_consumed = true;
                                    }

                                    if text_line.ends_with("_1") { // has body

                                        let (sender, body) = Body::channel();
                                        body_writer = Some(sender);

                                        let req = lib::parse(&text_line, target).body(body).unwrap();
                                        client_request_future = Some(client.request(req));
                                    } else if text_line.ends_with("_2") { // no body

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
                                    } else if text_line.ends_with("_3") {  // body send completed

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
                                }
                                Message::Binary(_) => {
                                    if body_writer.as_mut().is_some() {
                                        let writer = body_writer.as_mut().unwrap();
                                        match writer.send_data(Bytes::from(msg.into_data())).await {
                                            Ok(_) => { println!("writer send data done") }
                                            Err(_) => { println!("writer send data err") }
                                        };
                                    }
                                }
                                Message::Ping(_) => {}
                                Message::Pong(_) => {}
                                Message::Close(_) => {}
                                Message::Frame(_) => {}
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

#[tokio::main]
async fn main() {
    // let config = Config {
    //     route: String::from("*"),
    //     target_uri: String::from("http://localhost:8080"),
    //     component_name: String::from("rust-testing-component"),
    //     router_uri_provider: || { vec![String::from("wss://localhost:12001")] },
    //     sliding_window: 2,
    // };
    // let mut connector = CrankerConnector::new(config);
    // connector.start().await;
    //
    // thread::sleep(Duration::from_secs(3000))


    println!("started");
    let mut interval = time::interval(Duration::from_secs(2));
    &interval.tick().await;

    let (tx, rx) = oneshot::channel();
    let mut isStarted = true;

    tx.send("cancel").unwrap();

    interval.reset();
}

#[cfg(test)]
mod tests {}
