use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

fn server(
    reply: Vec<u8>,
    delay: Duration,
) -> (String, mpsc::Receiver<Vec<u8>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/responses", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut captured = Vec::new();
        let mut buffer = [0u8; 2048];
        loop {
            let count = socket.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            captured.extend_from_slice(&buffer[..count]);
            if let Some(end) = captured.windows(4).position(|s| s == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&captured[..end]).to_lowercase();
                let length = header
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if captured.len() >= end + 4 + length {
                    break;
                }
            }
        }
        tx.send(captured).unwrap();
        if delay != Duration::ZERO && !reply.is_empty() {
            let end = reply.windows(4).position(|s| s == b"\r\n\r\n").unwrap() + 4;
            socket.write_all(&reply[..end]).unwrap();
            thread::sleep(delay);
            let _ = socket.write_all(&reply[end..]);
        } else {
            thread::sleep(delay);
            let _ = socket.write_all(&reply);
        }
    });
    (url, rx, worker)
}
#[test]
fn real_http_transport_posts_json_with_auth_and_limits_success_body() {
    let reply = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec();
    let (url, rx, worker) = server(reply, Duration::ZERO);
    let response = OpenAiHttp::new()
        .unwrap()
        .post_to(
            &url,
            &json!({"fixture":true}),
            &Secret::new("test-token".into()).unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(response.body, b"{}");
    let request = String::from_utf8(rx.recv().unwrap()).unwrap();
    assert!(request.starts_with("POST /v1/responses HTTP/1.1"));
    assert!(request
        .to_lowercase()
        .contains("authorization: bearer test-token"));
    assert!(request.ends_with(r#"{"fixture":true}"#));
    worker.join().unwrap();

    let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
    let mut reply = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    reply.extend(body);
    let (url, _rx, worker) = server(reply, Duration::ZERO);
    assert!(matches!(
        OpenAiHttp::new().unwrap().post_to(
            &url,
            &json!({}),
            &Secret::new("test-token".into()).unwrap(),
            Duration::from_secs(2)
        ),
        Err(VisionError::InvalidResponse)
    ));
    worker.join().unwrap();
}
#[test]
fn real_http_transport_times_out_and_does_not_follow_redirects() {
    let reply = b"HTTP/1.1 302 Found\r\nContent-Length: 0\r\nLocation: http://127.0.0.1:1/private\r\nConnection: close\r\n\r\n".to_vec();
    let (url, _rx, worker) = server(reply, Duration::ZERO);
    let response = OpenAiHttp::new()
        .unwrap()
        .post_to(
            &url,
            &json!({}),
            &Secret::new("test-token".into()).unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(response.status, 302);
    worker.join().unwrap();
    let (url, _rx, worker) = server(Vec::new(), Duration::from_millis(300));
    assert!(matches!(
        OpenAiHttp::new().unwrap().post_to(
            &url,
            &json!({}),
            &Secret::new("test-token".into()).unwrap(),
            Duration::from_millis(100)
        ),
        Err(VisionError::Timeout)
    ));
    worker.join().unwrap();
}

#[test]
fn successful_headers_then_stalled_body_are_a_timeout() {
    let reply = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec();
    let (url, _rx, worker) = server(reply, Duration::from_millis(300));
    let result = OpenAiHttp::new().unwrap().post_to(
        &url,
        &json!({}),
        &Secret::new("test-token".into()).unwrap(),
        Duration::from_millis(100),
    );
    worker.join().unwrap();
    assert!(matches!(result, Err(VisionError::Timeout)));
}
