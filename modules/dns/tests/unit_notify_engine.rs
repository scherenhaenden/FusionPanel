#![forbid(unsafe_code)]

use dns::notify_engine::*;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[test]
fn test_notify_format() {
    let wire = NotifyEngine::format_notify_request(1234, "example.com");
    assert!(wire.len() >= 12);
    // check ID
    assert_eq!(wire[0], 0x04);
    assert_eq!(wire[1], 0xD2);
    // check flags: QR=0, Opcode=4, AA=1 -> 0x24
    assert_eq!(wire[2], 0x24);
    assert_eq!(wire[3], 0x00);
    // check counts
    assert_eq!(u16::from_be_bytes([wire[4], wire[5]]), 1); // QDCOUNT
    assert_eq!(u16::from_be_bytes([wire[6], wire[7]]), 0); // ANCOUNT
    assert_eq!(u16::from_be_bytes([wire[8], wire[9]]), 0); // NSCOUNT
    assert_eq!(u16::from_be_bytes([wire[10], wire[11]]), 0); // ARCOUNT

    // example.com
    let qname_start = 12;
    assert_eq!(wire[qname_start], 7);
    assert_eq!(&wire[qname_start+1..qname_start+8], b"example");
    assert_eq!(wire[qname_start+8], 3);
    assert_eq!(&wire[qname_start+9..qname_start+12], b"com");
    assert_eq!(wire[qname_start+12], 0);

    // QTYPE = SOA (6)
    assert_eq!(u16::from_be_bytes([wire[qname_start+13], wire[qname_start+14]]), 6);
    // QCLASS = IN (1)
    assert_eq!(u16::from_be_bytes([wire[qname_start+15], wire[qname_start+16]]), 1);
}

#[test]
fn test_engine_schedule_and_ack() {
    let mut engine = NotifyEngine::new(3, Duration::from_secs(1));
    let addr: SocketAddr = "127.0.0.1:53".parse().unwrap();
    let id = engine.schedule_notify("example.com".to_string(), addr);

    let now = Instant::now();
    let ready = engine.process_queues(now);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].0.id, id);

    // build an ACK
    let mut ack = ready[0].1.clone();
    ack[2] |= 0x80; // set QR=1
    // Opcode=4, RCODE=0 already ok

    let res = engine.handle_ack(&ack);
    assert!(res.is_ok());
    assert_eq!(res.unwrap().id, id);

    assert_eq!(engine.get_acknowledged_tasks().len(), 1);
    assert_eq!(engine.get_failed_tasks().len(), 0);
}

#[test]
fn test_engine_timeout_and_retry() {
    let mut engine = NotifyEngine::new(2, Duration::from_secs(1));
    let addr: SocketAddr = "127.0.0.1:53".parse().unwrap();
    let id = engine.schedule_notify("example.com".to_string(), addr);

    let now = Instant::now();
    let ready = engine.process_queues(now);
    assert_eq!(ready.len(), 1);

    // advance time by 1 sec
    let now1 = now + Duration::from_secs(1);
    let ready2 = engine.process_queues(now1);
    assert_eq!(ready2.len(), 1); // retry 1
    assert_eq!(ready2[0].0.retries, 1);

    // advance by 2 secs (base_timeout * 2^(1))
    let now2 = now1 + Duration::from_secs(2);
    let ready3 = engine.process_queues(now2);
    assert_eq!(ready3.len(), 1); // retry 2
    assert_eq!(ready3[0].0.retries, 2);

    // advance by 4 secs, should fail because max_retries = 2
    let now3 = now2 + Duration::from_secs(4);
    let ready4 = engine.process_queues(now3);
    assert_eq!(ready4.len(), 0);

    assert_eq!(engine.get_failed_tasks().len(), 1);
    assert_eq!(engine.get_failed_tasks()[0].id, id);
}
