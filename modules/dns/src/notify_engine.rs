#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyStatus {
    Pending,
    Acknowledged,
    Failed,
}

#[derive(Debug, Clone)]
pub struct NotifyTask {
    pub id: u16,
    pub zone: String,
    pub slave_addr: SocketAddr,
    pub retries: u32,
    pub next_attempt: Instant,
}

pub struct NotifyEngine {
    queue: VecDeque<NotifyTask>,
    in_flight: HashMap<u16, NotifyTask>,
    failed: Vec<NotifyTask>,
    acknowledged: Vec<NotifyTask>,
    next_id: u16,
    max_retries: u32,
    base_timeout: Duration,
}

impl NotifyEngine {
    pub fn new(max_retries: u32, base_timeout: Duration) -> Self {
        Self {
            queue: VecDeque::new(),
            in_flight: HashMap::new(),
            failed: Vec::new(),
            acknowledged: Vec::new(),
            next_id: 1,
            max_retries,
            base_timeout,
        }
    }

    pub fn schedule_notify(&mut self, zone: String, slave_addr: SocketAddr) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        let task = NotifyTask {
            id,
            zone,
            slave_addr,
            retries: 0,
            next_attempt: Instant::now(),
        };
        self.queue.push_back(task);
        id
    }

    pub fn process_queues(&mut self, now: Instant) -> Vec<(NotifyTask, Vec<u8>)> {
        let mut ready = Vec::new();

        // Check in-flight tasks for timeouts
        let mut timed_out = Vec::new();
        for (&id, task) in &self.in_flight {
            if now >= task.next_attempt {
                timed_out.push(id);
            }
        }

        for id in timed_out {
            if let Some(mut task) = self.in_flight.remove(&id) {
                task.retries += 1;
                if task.retries <= self.max_retries {
                    task.next_attempt = now;
                    self.queue.push_back(task);
                } else {
                    self.failed.push(task);
                }
            }
        }

        // Process queue
        let mut i = 0;
        while i < self.queue.len() {
            if self.queue[i].next_attempt <= now {
                let mut task = self.queue.remove(i).unwrap();
                let wire_msg = Self::format_notify_request(task.id, &task.zone);

                let delay = self.base_timeout * 2u32.pow(task.retries);
                task.next_attempt = now + delay;

                ready.push((task.clone(), wire_msg));
                self.in_flight.insert(task.id, task);
            } else {
                i += 1;
            }
        }

        ready
    }

    pub fn handle_ack(&mut self, response_wire: &[u8]) -> Result<NotifyTask, &'static str> {
        if response_wire.len() < 12 {
            return Err("Response too short");
        }

        let id = u16::from_be_bytes([response_wire[0], response_wire[1]]);
        let flags1 = response_wire[2];
        let flags2 = response_wire[3];

        let qr = (flags1 & 0x80) != 0;
        let opcode = (flags1 & 0x78) >> 3;
        let rcode = flags2 & 0x0F;

        if !qr {
            return Err("Not a response");
        }
        if opcode != 4 {
            return Err("Not a NOTIFY opcode");
        }
        if rcode != 0 {
            return Err("RCODE is not NoError");
        }

        if let Some(task) = self.in_flight.remove(&id) {
            self.acknowledged.push(task.clone());
            Ok(task)
        } else {
            Err("Unknown or already processed ID")
        }
    }

    pub fn format_notify_request(id: u16, zone: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&id.to_be_bytes());
        buf.push(0x24); // QR=0, Opcode=4, AA=1, TC=0, RD=0
        buf.push(0x00); // RA=0, Z=0, RCODE=0
        buf.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
        buf.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
        buf.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
        buf.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT

        // QNAME
        for part in zone.split('.') {
            if part.is_empty() {
                continue;
            }
            let len = std::cmp::min(part.len(), 63);
            buf.push(len as u8);
            buf.extend_from_slice(&part.as_bytes()[..len]);
        }
        buf.push(0);

        // QTYPE = SOA (6)
        buf.extend_from_slice(&6u16.to_be_bytes());
        // QCLASS = IN (1)
        buf.extend_from_slice(&1u16.to_be_bytes());

        buf
    }

    pub fn get_failed_tasks(&self) -> &[NotifyTask] {
        &self.failed
    }

    pub fn get_acknowledged_tasks(&self) -> &[NotifyTask] {
        &self.acknowledged
    }

    pub fn clear_completed(&mut self) {
        self.failed.clear();
        self.acknowledged.clear();
    }
}
