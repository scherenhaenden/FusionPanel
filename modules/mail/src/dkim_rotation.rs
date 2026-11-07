#![forbid(unsafe_code)]

use crate::dns_records::{DkimKeyPair, DkimKeyType, DnsGenerator, DnsRecord};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct DkimSelectorLifecycle {
    pub domain: String,
    pub active_selector: String,
    pub retired_selectors: Vec<String>,
}

impl DkimSelectorLifecycle {
    pub fn new(domain: &str, initial_selector: &str) -> Self {
        Self {
            domain: domain.to_string(),
            active_selector: initial_selector.to_string(),
            retired_selectors: Vec::new(),
        }
    }

    pub fn rotate_keys(&mut self, key_type: DkimKeyType) -> Result<DkimKeyPair, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Simple YYYYMM generation logic without external crate
        // Approximate to avoid shelling out `date`
        let secs_per_day = 86400;
        let days = now / secs_per_day;
        let mut year = 1970;
        let mut remaining_days = days;

        loop {
            let days_in_year = if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                366
            } else {
                365
            };
            if remaining_days >= days_in_year {
                remaining_days -= days_in_year;
                year += 1;
            } else {
                break;
            }
        }

        let days_in_month = [
            31,
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];

        let mut month = 0;
        for (i, dim) in days_in_month.iter().enumerate() {
            if remaining_days >= *dim {
                remaining_days -= *dim;
            } else {
                month = i + 1;
                break;
            }
        }

        let new_selector_base = format!("{:04}{:02}", year, month);
        let mut new_selector = new_selector_base.clone();

        if new_selector == self.active_selector || self.retired_selectors.contains(&new_selector) {
            new_selector = format!("{}-{}", new_selector, now);
        }

        let dns_gen = DnsGenerator::new(&self.domain)?;
        let key_pair = dns_gen.generate_dkim(&new_selector, key_type)?;

        self.retired_selectors.push(self.active_selector.clone());
        self.active_selector = new_selector;

        Ok(key_pair)
    }

    pub fn get_all_txt_records(
        &self,
        current_key_pair: &DkimKeyPair,
        old_key_pairs: &[DkimKeyPair],
    ) -> Vec<DnsRecord> {
        let mut records = Vec::new();
        records.push(current_key_pair.txt_record.clone());
        for kp in old_key_pairs {
            if self.retired_selectors.contains(&kp.selector) {
                records.push(kp.txt_record.clone());
            }
        }
        records
    }
}
