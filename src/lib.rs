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
}
