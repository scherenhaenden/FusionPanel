#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClamAVError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid response from daemon: {0}")]
    InvalidResponse(String),
    #[error("Scan error: {0}")]
    ScanError(String),
}

#[derive(Debug, PartialEq)]
pub enum ScanResult {
    Clean,
    Infected(String),
}

pub struct ClamAVClient {
    address: String,
    timeout: Duration,
}

impl ClamAVClient {
    pub fn new<T: Into<String>>(address: T) -> Self {
        Self {
            address: address.into(),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn connect(&self) -> Result<TcpStream, ClamAVError> {
        let stream = TcpStream::connect(&self.address)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        Ok(stream)
    }

    pub fn ping(&self) -> Result<bool, ClamAVError> {
        let mut stream = self.connect()?;
        stream.write_all(b"zPING\0")?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        Ok(response.trim_end_matches('\0') == "PONG")
    }

    pub fn version(&self) -> Result<String, ClamAVError> {
        let mut stream = self.connect()?;
        stream.write_all(b"zVERSION\0")?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        let response = response.trim_end_matches('\0');
        if response.starts_with("ClamAV ") {
            Ok(response.to_string())
        } else {
            Err(ClamAVError::InvalidResponse(response.to_string()))
        }
    }

    pub fn scan_stream<R: Read>(&self, mut reader: R) -> Result<ScanResult, ClamAVError> {
        let mut stream = self.connect()?;
        stream.write_all(b"zINSTREAM\0")?;

        let mut buffer = [0u8; 8192];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                // Send zero-length chunk to indicate end of stream
                stream.write_all(&[0, 0, 0, 0])?;
                break;
            }

            // Write chunk size as 32-bit network byte order
            let size = (n as u32).to_be_bytes();
            stream.write_all(&size)?;
            // Write chunk data
            stream.write_all(&buffer[..n])?;
        }

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        let response = response.trim_end_matches('\0');

        if response.ends_with(" OK") {
            Ok(ScanResult::Clean)
        } else if let Some(found) = response.strip_suffix(" FOUND") {
            // "stream: Eicar-Test-Signature FOUND"
            let prefix = "stream: ";
            let virus_name = if let Some(stripped) = found.strip_prefix(prefix) {
                stripped
            } else {
                found
            };
            Ok(ScanResult::Infected(virus_name.to_string()))
        } else if let Some(err) = response.strip_suffix(" ERROR") {
            Err(ClamAVError::ScanError(err.to_string()))
        } else {
            Err(ClamAVError::InvalidResponse(response.to_string()))
        }
    }
}
