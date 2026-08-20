//! Relay-backed tests for webhook `project_channel_by_repository` routing.
//!
//! These tests require a running relay. They are `#[ignore]` so `cargo test`
//! does not fail in CI when the relay is not available.
//!
//! ```text
//! RELAY_URL=ws://localhost:3000 cargo test -p buzz-test-client \
//!   --test e2e_workflow_project_channel_routing -- --ignored
//! ```

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use buzz_test_client::BuzzTestClient;
use nostr::{EventBuilder, Keys, Kind, Tag};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn relay_url() -> String {
    std::env::var("RELAY_URL").unwrap_or_else(|_| "ws://localhost:3000".to_string())
}

fn relay_http_url() -> String {
    relay_url()
        .replace("wss://", "https://")
        .replace("ws://", "http://")
        .trim_end_matches('/')
        .to_string()
}

/// A short unique suffix so concurrent runs never collide on a `d` tag.
fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().to_string()[..8])
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// NIP-98 `Authorization` header for an authenticated HTTP POST.
#[allow(dead_code)]
fn nip98_post_header(keys: &Keys, url: &str, body: &str) -> String {
    let event = EventBuilder::new(Kind::Custom(27_235), "")
        .tags(vec![
            Tag::parse(["u", url]).unwrap(),
            Tag::parse(["method", "POST"]).unwrap(),
            Tag::parse(["payload", &sha256_hex(body.as_bytes())]).unwrap(),
            Tag::parse(["nonce", &Uuid::new_v4().to_string()]).unwrap(),
        ])
        .sign_with_keys(keys)
        .expect("sign NIP-98 event");
    format!(
        "Nostr {}",
        BASE64.encode(serde_json::to_string(&event).expect("serialize NIP-98 event"))
    )
}

/// Submit a signed event via the HTTP bridge (`POST /events`).
async fn submit_event(keys: &Keys, event: nostr::Event) -> serde_json::Value {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/events", relay_http_url()))
        .header("X-Pubkey", keys.public_key().to_hex())
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&event).expect("serialize event"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST /events failed: {e}"));
    let status = resp.status();
    let body = resp.text().await.expect("read /events body");
    let value: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("parse /events JSON: {e} (body: {body})"));
    if status.as_u16() == 200 {
        value
    } else {
        serde_json::json!({
            "accepted": false,
            "message": value["error"].as_str().unwrap_or(&body),
        })
    }
}

/// Create an `open` stream channel; the signer is bootstrapped as owner-member.
async fn create_channel(keys: &Keys, name_prefix: &str) -> Uuid {
    let channel_uuid = Uuid::new_v4();
    let channel_name = unique(name_prefix);
    let event = EventBuilder::new(Kind::Custom(9007), "")
        .tags(vec![
            Tag::parse(["h", &channel_uuid.to_string()]).unwrap(),
            Tag::parse(["name", &channel_name]).unwrap(),
            Tag::parse(["channel_type", "stream"]).unwrap(),
            Tag::parse(["visibility", "open"]).unwrap(),
        ])
        .sign_with_keys(keys)
        .unwrap();
    let body = submit_event(keys, event).await;
    assert!(
        body["accepted"].as_bool().unwrap_or(false),
        "create-channel not accepted: {body}"
    );
    channel_uuid
}

struct WorkflowSaveResult {
    accepted: bool,
    message: String,
    workflow_id: Option<Uuid>,
    webhook_secret: Option<String>,
}

/// Publish a client-UUID kind `30620` workflow definition over the authenticated
/// WebSocket. Response fields are populated only when the save is accepted.
async fn define_webhook_workflow_raw(
    keys: &Keys,
    home_channel: Uuid,
    yaml: &str,
) -> WorkflowSaveResult {
    let workflow_id = Uuid::new_v4();
    let event = EventBuilder::new(Kind::Custom(30620), yaml)
        .tags(vec![
            Tag::parse(["d", &workflow_id.to_string()]).unwrap(),
            Tag::parse(["h", &home_channel.to_string()]).unwrap(),
        ])
        .sign_with_keys(keys)
        .expect("sign workflow def");

    let mut client = BuzzTestClient::connect(&relay_url(), keys)
        .await
        .expect("connect");
    let ok = client
        .send_event(event)
        .await
        .expect("publish workflow def");
    client.disconnect().await.expect("disconnect");

    if !ok.accepted {
        return WorkflowSaveResult {
            accepted: false,
            message: ok.message,
            workflow_id: None,
            webhook_secret: None,
        };
    }

    let json_part = ok.message.strip_prefix("response:").unwrap_or_else(|| {
        panic!(
            "accepted workflow save missing `response:` prefix: {:?}",
            ok.message
        )
    });
    let resp: serde_json::Value = serde_json::from_str(json_part)
        .unwrap_or_else(|e| panic!("parse workflow save response json: {e} ({json_part:?})"));
    WorkflowSaveResult {
        accepted: true,
        message: ok.message,
        workflow_id: resp["workflow_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok()),
        webhook_secret: resp["webhook_secret"].as_str().map(str::to_string),
    }
}

#[tokio::test]
#[ignore = "requires a running relay"]
async fn alias_target_must_be_live_at_save() {
    let keys = nostr::Keys::generate();
    let home_channel = create_channel(&keys, "alias-home").await;
    let alias_key = "hs";
    let missing_target = format!(
        "30617:{}:missing-harness-service",
        keys.public_key().to_hex()
    );
    let yaml = format!(
        "name: Alias Save Rejection\n\
         trigger:\n  on: webhook\n\
         routing:\n  mode: project_channel_by_repository\n\
         \x20 aliases:\n    {alias_key}: {missing_target}\n\
         steps:\n  - id: notify\n    action: send_message\n    text: hi\n"
    );
    let result = define_webhook_workflow_raw(&keys, home_channel, &yaml).await;
    assert!(!result.accepted);
    assert!(result.workflow_id.is_none());
    assert!(result.webhook_secret.is_none());
    assert!(result.message.contains(alias_key));
    assert!(!result.message.contains("30617:"));
    assert!(!result.message.contains("missing-harness-service"));
}
