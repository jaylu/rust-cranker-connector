use http::request::Builder;
use http::{Method, Request as hyper_request};
use std::str::FromStr;

pub fn parse(input: &str, target: &str) -> Builder {
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
            None => (),
        }
    }

    let uri = format!("{}{}", target, path);
    println!("uri={}, method={}", uri, method);
    return request_builder.method(Method::from_str(method).unwrap()).uri(uri);
}


pub fn back_out(attempts: u32) -> u64 {
    
    static MAX_BACK_OUT: u64 = 10000;
    static BASE:u64 = 500;
    
    let increase = if attempts > 13 {
        MAX_BACK_OUT
    } else {
        let result = 0x2u64.pow(attempts);
        if result > MAX_BACK_OUT {
            MAX_BACK_OUT
        } else {
            result
        }
    };
    return BASE + increase;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::{Borrow, BorrowMut};
    use std::cell::{RefCell, RefMut};
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::slice::RChunks;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::time;

    #[test]
    fn test_back_out() {
        assert_eq!(back_out(1), 502);
        assert_eq!(back_out(2), 504);
        assert_eq!(back_out(10), 1524);
        assert_eq!(back_out(14), 10500);
        assert_eq!(back_out(2343), 10500);
    }

    #[test]
    fn test_parse() {
        let input = "POST /post-msg HTTP/1.1\nUser-Agent:curl/7.64.1\nHost:localhost:9443\nAccept:*/*\nContent-Length:52\nContent-Type:application/json\nForwarded:for=0:0:0:0:0:0:0:1;proto=https;host=localhost:9443;by=0:0:0:0:0:0:0:1\nX-Forwarded-For:0:0:0:0:0:0:0:1\nX-Forwarded-Proto:https\nX-Forwarded-Host:localhost:9443\nX-Forwarded-Server:0:0:0:0:0:0:0:1\n\n_1";
        let builder = parse(input, "http://localhost:8080");
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

    #[test]
    fn test_vec_chunks() {
        let sample: Vec<&str> = vec!["a", "b", "c", "d", "e"];
        let collects: Vec<&[&str]> = sample.chunks(2).collect();
        for item in collects {
            println!("output: {:?}", item);
        }
    }

    #[test]
    fn test_option() {
        let mut v: Option<String> = None;
        println!("output: {:?}", &v);

        v.replace(String::from("Hello"));
        println!("output: {:?}", &v);
    }

    #[test]
    fn test_smart_pointer_arc() {
        //--nocapture
        let array = vec!["a", "b", "c"];
        let rc = Arc::new(array);

        let rc1 = Arc::clone(&rc);
        let rc2 = Arc::clone(&rc);

        let mut handles = vec![];

        let handle_1 = thread::spawn(move || println!("{:?}", rc1));
        handles.push(handle_1);

        let handle_2 = thread::spawn(move || println!("{:?}", rc2));
        handles.push(handle_2);

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_smart_pointer_arc_with_mutex() {
        //--nocapture
        let array = vec!["a", "b", "c"];
        let rc = Arc::new(Mutex::new(array));

        let rc1 = Arc::clone(&rc);
        let rc2 = Arc::clone(&rc);

        let mut handles = vec![];

        let handle_1 = thread::spawn(move || {
            println!("rc1: {:?}", rc1);
            rc1.lock().unwrap().push("d");
        });
        handles.push(handle_1);

        let handle_2 = thread::spawn(move || {
            println!("rc2: {:?}", rc2);
            rc2.lock().unwrap().push("e");
        });
        handles.push(handle_2);

        for handle in handles {
            handle.join().unwrap();
        }

        println!("rc: {:?}", rc);
    }

    #[test]
    fn test_smart_pointer_box() {
        //--nocapture
        let array = vec!["a", "b", "c"];
        let mut value = Box::new(array);

        println!("box before update: {:?}", &value);

        value.push("d");
        value.push("e");

        println!("box after update: {:?}", &value);
    }

    #[test]
    fn test_tokio_select() {
        //     tokio::select! {
        //     _ = async {
        //         while &isStarted {
        //             &interval.tick().await;
        //         }
        //         println!("complete")
        //     } => {},
        //     _cancel = rx => {
        //         isStarted = false;
        //         println!("cancel")
        //     }
        // }
    }

    #[tokio::test]
    async fn tokio_broadcast_test() {
        // one producer, multiple subscriber scenario
        let (tx, _) = broadcast::channel::<String>(16);
        let mut rx_1 = tx.subscribe();
        let mut rx_2 = tx.subscribe();

        let handle_1 = tokio::spawn(async move {
            assert_eq!(rx_1.recv().await.is_err(), true);
            println!("rx_1 done")
        });

        let handle_2 = tokio::spawn(async move {
            assert_eq!(rx_2.recv().await.is_err(), true);
            println!("rx_2 done")
        });

        drop(tx);
        let _ = handle_1.await;
        let _ = handle_2.await;
    }

    #[tokio::test]
    async fn tokio_broadcast_select_test_drop() {
        // one producer, multiple subscriber scenario
        let (tx, _) = broadcast::channel::<()>(16);
        let mut rx_1 = tx.subscribe();
        let is_break = Arc::new(AtomicBool::new(false));
        let is_break_clone = is_break.clone();

        let handle_1 = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    rec = rx_1.recv() => {
                        assert_eq!(rec.is_err(), true);
                        break;
                    }
                }
            }

            is_break.store(true, Ordering::SeqCst);
        });

        assert_eq!(is_break_clone.load(Ordering::Acquire), false);
        drop(tx);
        let _ = handle_1.await;
        assert_eq!(is_break_clone.load(Ordering::Acquire), true);
    }

    #[tokio::test]
    async fn tokio_broadcast_select_test_send() {
        // one producer, multiple subscriber scenario
        let (tx, _) = broadcast::channel::<()>(16);
        let mut rx_1 = tx.subscribe();
        let is_break = Arc::new(AtomicBool::new(false));
        let is_break_clone = is_break.clone();

        let handle_1 = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    rec = rx_1.recv() => {
                        assert_eq!(rec.is_err(), false);
                        break;
                    }
                }
            }

            is_break.store(true, Ordering::SeqCst);
        });

        assert_eq!(is_break_clone.load(Ordering::Acquire), false);
        let _ = tx.send(());
        let _ = handle_1.await;
        assert_eq!(is_break_clone.load(Ordering::Acquire), true);
    }
}
