#![forbid(unsafe_code)]

use crate::domain::{DomainName, RecordType, Zone};
use std::fmt::Write;

pub fn generate_zone_file(
    zone: &Zone,
    primary_ns: &DomainName,
    admin_email: &str,
    current_date_str: &str,
    current_serial: Option<u32>,
) -> Result<String, std::fmt::Error> {
    let mut out = String::new();

    let rname = admin_email.replace('@', ".");
    let serial = next_serial(current_date_str, current_serial);

    writeln!(out, "$ORIGIN {}.", zone.name().as_str())?;

    writeln!(out, "@ 3600 IN SOA {}. {}. (", primary_ns.as_str(), rname)?;
    writeln!(out, "    {} ; serial", serial)?;
    writeln!(out, "    7200       ; refresh (2 hours)")?;
    writeln!(out, "    3600       ; retry (1 hour)")?;
    writeln!(out, "    1209600    ; expire (2 weeks)")?;
    writeln!(out, "    3600       ; minimum (1 hour)")?;
    writeln!(out, ")")?;

    for record in zone.records() {
        let name_str = record.name().as_str();
        let zone_str = zone.name().as_str();

        let relative_name = if name_str == zone_str {
            "@".to_string()
        } else if name_str.ends_with(&format!(".{}", zone_str)) {
            name_str[..name_str.len() - zone_str.len() - 1].to_string()
        } else {
            format!("{}.", name_str)
        };

        match record.record_type() {
            RecordType::A => writeln!(
                out,
                "{} {} IN A {}",
                relative_name,
                record.ttl(),
                record.data()
            )?,
            RecordType::AAAA => writeln!(
                out,
                "{} {} IN AAAA {}",
                relative_name,
                record.ttl(),
                record.data()
            )?,
            RecordType::CNAME => {
                let mut data = record.data().to_string();
                if !data.ends_with('.') {
                    data.push('.');
                }
                writeln!(out, "{} {} IN CNAME {}", relative_name, record.ttl(), data)?
            }
            RecordType::MX => writeln!(
                out,
                "{} {} IN MX {}",
                relative_name,
                record.ttl(),
                record.data()
            )?,
            RecordType::TXT => {
                let data = record.data();
                let escaped = data.replace('"', "\\\"");
                writeln!(
                    out,
                    "{} {} IN TXT \"{}\"",
                    relative_name,
                    record.ttl(),
                    escaped
                )?
            }
            #[allow(unreachable_patterns)]
            _ => {
                // Ignore SRV, NS if they get added to Enum but not to match arm yet
                writeln!(out, "; Unsupported record type for {}", relative_name)?
            }
        }
    }

    Ok(out)
}

pub fn next_serial(current_date_str: &str, current_serial: Option<u32>) -> u32 {
    let base: u32 = format!("{}00", current_date_str).parse().unwrap_or(0);

    match current_serial {
        Some(serial) => {
            let serial_str = serial.to_string();
            if serial_str.starts_with(current_date_str) && serial_str.len() == 10 {
                let revision = serial % 100;
                if revision < 99 {
                    serial + 1
                } else {
                    serial
                }
            } else {
                base + 1
            }
        }
        None => base + 1,
    }
}
