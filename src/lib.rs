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
            None => ()
        }
    }

    let uri = format!("{}{}", target, path);
    println!("uri={}", uri);
    return request_builder
        .method(Method::from_str(method).unwrap())
        .uri(uri);
}



#[cfg(test)]
mod tests {
    use std::borrow::{Borrow, BorrowMut};
    use std::cell::{RefCell, RefMut};
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::slice::RChunks;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use super::*;

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

        let handle_1 = thread::spawn(move || {
            println!("{:?}", rc1)
        });
        handles.push(handle_1);

        let handle_2 = thread::spawn(move || {
            println!("{:?}", rc2)
        });
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

}
