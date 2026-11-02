use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct ComposeFile {
    pub version: Option<String>,
    #[serde(default)]
    pub services: HashMap<String, Service>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Service {
    pub image: Option<String>,
    #[serde(default)]
    pub ports: Vec<PortMapping>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub privileged: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PublishedPort {
    String(String),
    Int(u16),
}

impl fmt::Display for PublishedPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PublishedPort::String(s) => write!(f, "{}", s),
            PublishedPort::Int(i) => write!(f, "{}", i),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PortMapping {
    String(String),
    Object {
        target: u16,
        published: Option<PublishedPort>,
        protocol: Option<String>,
        mode: Option<String>,
    },
}
