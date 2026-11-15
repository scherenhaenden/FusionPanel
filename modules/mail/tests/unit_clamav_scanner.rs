#![forbid(unsafe_code)]

use mail::clamav_scanner::{ClamAVClient, ClamAVError, ScanResult};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn spawn_mock_server<F>(handler: F) -> String
where
    F: FnOnce(TcpStream) + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handler(stream);
        }
    });
    addr
}

#[test]
fn test_ping_success() {
    let addr = spawn_mock_server(|mut stream| {
        let mut buf = [0; 6];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"zPING\0");
        stream.write_all(b"PONG\0").unwrap();
    });

    let client = ClamAVClient::new(addr);
    assert_eq!(client.ping().unwrap(), true);
}

#[test]
fn test_ping_invalid_response() {
    let addr = spawn_mock_server(|mut stream| {
        let mut buf = [0; 6];
        stream.read_exact(&mut buf).unwrap();
        stream.write_all(b"INVALID\0").unwrap();
    });

    let client = ClamAVClient::new(addr);
    assert_eq!(client.ping().unwrap(), false);
}

#[test]
fn test_version_success() {
    let addr = spawn_mock_server(|mut stream| {
        let mut buf = [0; 9];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"zVERSION\0");
        stream.write_all(b"ClamAV 0.103.0/26000\0").unwrap();
    });

    let client = ClamAVClient::new(addr);
    assert_eq!(client.version().unwrap(), "ClamAV 0.103.0/26000");
}

#[test]
fn test_version_invalid() {
    let addr = spawn_mock_server(|mut stream| {
        let mut buf = [0; 9];
        stream.read_exact(&mut buf).unwrap();
        stream.write_all(b"Some other AV\0").unwrap();
    });

    let client = ClamAVClient::new(addr);
    assert!(matches!(
        client.version(),
        Err(ClamAVError::InvalidResponse(_))
    ));
}

#[test]
fn test_scan_clean() {
    let addr = spawn_mock_server(|mut stream| {
        let mut buf = [0; 10];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"zINSTREAM\0");

        let mut size_buf = [0; 4];
        stream.read_exact(&mut size_buf).unwrap();
        let size = u32::from_be_bytes(size_buf);
        assert_eq!(size, 4);

        let mut data = vec![0; size as usize];
        stream.read_exact(&mut data).unwrap();
        assert_eq!(data, b"test");

        stream.read_exact(&mut size_buf).unwrap();
        assert_eq!(u32::from_be_bytes(size_buf), 0);

        stream.write_all(b"stream: OK\0").unwrap();
    });

    let client = ClamAVClient::new(addr);
    let data = b"test";
    let result = client.scan_stream(&data[..]).unwrap();
    assert_eq!(result, ScanResult::Clean);
}

#[test]
fn test_scan_infected() {
    let addr = spawn_mock_server(|mut stream| {
        let mut buf = [0; 10];
        stream.read_exact(&mut buf).unwrap();

        let mut size_buf = [0; 4];
        stream.read_exact(&mut size_buf).unwrap();
        let size = u32::from_be_bytes(size_buf);
        let mut data = vec![0; size as usize];
        stream.read_exact(&mut data).unwrap();

        stream.read_exact(&mut size_buf).unwrap();

        stream.write_all(b"stream: Eicar-Test-Signature FOUND\0").unwrap();
    });

    let client = ClamAVClient::new(addr);
    let data = b"eicar_test_string";
    let result = client.scan_stream(&data[..]).unwrap();
    assert_eq!(result, ScanResult::Infected("Eicar-Test-Signature".to_string()));
}

#[test]
fn test_scan_error() {
    let addr = spawn_mock_server(|mut stream| {
        let mut buf = [0; 10];
        stream.read_exact(&mut buf).unwrap();

        let mut size_buf = [0; 4];
        stream.read_exact(&mut size_buf).unwrap();
        let size = u32::from_be_bytes(size_buf);
        let mut data = vec![0; size as usize];
        stream.read_exact(&mut data).unwrap();

        stream.read_exact(&mut size_buf).unwrap();

        stream.write_all(b"stream: Size limit reached ERROR\0").unwrap();
    });

    let client = ClamAVClient::new(addr);
    let data = b"too_large_data";
    let result = client.scan_stream(&data[..]);
    assert!(matches!(result, Err(ClamAVError::ScanError(msg)) if msg == "stream: Size limit reached"));
}
