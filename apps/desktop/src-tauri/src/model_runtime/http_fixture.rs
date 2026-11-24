use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread::{self, JoinHandle},
    time::Duration,
};

pub fn serve(
    count: usize,
    mut reply: impl FnMut(usize, &str, Value) -> (u16, Value) + Send + 'static,
) -> (reqwest::Url, JoinHandle<()>) {
    serve_with_authorization(count, move |sequence, path, _, body| {
        reply(sequence, path, body)
    })
}

pub fn serve_with_authorization(
    count: usize,
    mut reply: impl FnMut(usize, &str, Option<&str>, Value) -> (u16, Value) + Send + 'static,
) -> (reqwest::Url, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    let task = thread::spawn(move || {
        for sequence in 0..count {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&mut socket);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap().to_owned();
            let mut length = 0;
            let mut authorization = None;
            loop {
                line.clear();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((field, value)) = line.split_once(':') {
                    if field.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                    if field.eq_ignore_ascii_case("authorization") {
                        authorization = Some(value.trim().to_owned());
                    }
                }
            }
            let mut buffer = vec![0; length];
            reader.read_exact(&mut buffer).unwrap();
            let (code, body) = reply(
                sequence,
                &path,
                authorization.as_deref(),
                serde_json::from_slice(&buffer).unwrap_or(Value::Null),
            );
            let body = body.to_string();
            write!(socket, "HTTP/1.1 {code} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    (url, task)
}
