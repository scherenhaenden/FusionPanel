#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::time::Duration;

use monitoring::alert_notifier::{AlertNotifier, NotificationChannel};
use monitoring::incident::Incident;

struct MockNotificationChannel {
    pub sent_incidents: RefCell<Vec<Incident>>,
}

impl MockNotificationChannel {
    pub fn new() -> Self {
        Self {
            sent_incidents: RefCell::new(Vec::new()),
        }
    }
}

impl NotificationChannel for MockNotificationChannel {
    fn send(&self, incident: &Incident) -> Result<(), String> {
        self.sent_incidents.borrow_mut().push(incident.clone());
        Ok(())
    }
}

#[test]
fn test_dispatch_open_incident() {
    let channel = MockNotificationChannel::new();
    let mut notifier = AlertNotifier::new(channel, Duration::from_secs(60));

    let incident = Incident::new("rule_1".to_string());

    // First dispatch should succeed
    let dispatched = notifier.dispatch(&incident).unwrap();
    assert!(dispatched);

    // Next dispatch immediately should be blocked by cooldown
    let dispatched = notifier.dispatch(&incident).unwrap();
    assert!(!dispatched);
}

#[test]
fn test_dispatch_bypasses_cooldown_for_ack_and_resolved() {
    let channel = MockNotificationChannel::new();
    let mut notifier = AlertNotifier::new(channel, Duration::from_secs(60));

    let mut incident = Incident::new("rule_1".to_string());

    // First dispatch (Open)
    notifier.dispatch(&incident).unwrap();

    // Second dispatch (Open) - should be blocked
    let dispatched = notifier.dispatch(&incident).unwrap();
    assert!(!dispatched);

    // Acknowledge incident
    incident.acknowledge().unwrap();

    // Dispatch (Acknowledged) - should bypass cooldown and send
    let dispatched = notifier.dispatch(&incident).unwrap();
    assert!(dispatched);

    // Resolve incident
    incident.resolve().unwrap();

    // Dispatch (Resolved) - should bypass cooldown and send
    let dispatched = notifier.dispatch(&incident).unwrap();
    assert!(dispatched);
}

#[test]
fn test_resolved_resets_cooldown() {
    let channel = MockNotificationChannel::new();
    let mut notifier = AlertNotifier::new(channel, Duration::from_secs(60));

    let mut incident = Incident::new("rule_1".to_string());

    // First dispatch (Open)
    notifier.dispatch(&incident).unwrap();

    // Resolve incident
    incident.resolve().unwrap();
    notifier.dispatch(&incident).unwrap(); // This resets the cooldown

    // New incident for same rule should dispatch immediately because cooldown was reset
    let new_incident = Incident::new("rule_1".to_string());
    let dispatched = notifier.dispatch(&new_incident).unwrap();
    assert!(dispatched);
}
