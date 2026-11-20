//! Agents message each other (plan agent-messages.md, owner's room task 36): `message` shows in the
//! team chat as the sender's, wakes the member it names at most once, and counts toward the
//! hand-off limit, past which the chat pauses.

mod common;

use common::{TempDir, TestService};
use serde_json::{Value, json};

#[test]
fn the_hand_off_limit_is_a_setting_with_none_for_no_limit() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    assert_eq!(
        service.call("GET", "/api/settings").json()["handoff_limit"],
        50
    );
    let none = service.send("PATCH", "/api/settings", json!({ "handoff_limit": null }));
    assert_eq!(none.json()["handoff_limit"], Value::Null, "{}", none.body);
    let zero = service.send("PATCH", "/api/settings", json!({ "handoff_limit": 0 }));
    assert_ne!(zero.status, 200, "{}", zero.body);
    assert!(zero.json()["ref"].is_string(), "{}", zero.body);
    assert_eq!(
        service.call("GET", "/api/settings").json()["handoff_limit"],
        Value::Null
    );
}
