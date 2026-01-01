use ctor::{ctor, dtor};
use reqwest::multipart;
use reqwest::Client;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const DB_PATH: &str = "provisionr.db";

fn clear_database() {
    if let Ok(conn) = Connection::open(DB_PATH) {
        let _ = conn.execute("DELETE FROM rendered_templates", []);
    }
}

#[ctor]
fn setup() {
    clear_database();
}

#[dtor]
fn teardown() {
    clear_database();
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn server_url() -> String {
    std::env::var("PROVISIONR_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

fn url(path: &str) -> String {
    format!("{}{}", server_url(), path)
}

fn unique_name(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}-{}-{}", prefix, ts, count)
}

async fn upload_template(client: &Client, name: &str, content: &str) -> reqwest::Response {
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::text(content.to_string()).file_name("template.j2"),
    );

    client
        .post(url(&format!("/api/v1/template/{}", name)))
        .multipart(form)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_create_and_delete_template() {
    let client = Client::new();
    let name = unique_name("create-delete");

    // Create template using multipart
    let resp = upload_template(&client, &name, "Hello {{ name }}").await;
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // Delete template
    let resp = client
        .delete(url(&format!("/api/v1/template/{}", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // Verify template is gone
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=XX", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_set_and_render_with_values() {
    let client = Client::new();
    let name = unique_name("values");

    // Create template using multipart
    upload_template(&client, &name, "Hello {{ name }}, age {{ age }}").await;

    // Set values using raw body
    let resp = client
        .put(url(&format!("/api/v1/template/{}/values", name)))
        .body("name: World\nage: 42")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render and verify values are used
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=AA:BB:CC", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("World"), "Expected 'World' in: {}", body);
    assert!(body.contains("42"), "Expected '42' in: {}", body);

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_render_with_query_params() {
    let client = Client::new();
    let name = unique_name("query");

    // Create template using multipart
    upload_template(&client, &name, "Hello {{ name }}!").await;

    // Render with query params
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=AA:BB:CC&name=Integration", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "Hello Integration!");

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_caching_by_id_field() {
    let client = Client::new();
    let name = unique_name("cache");

    // Create template using multipart
    upload_template(&client, &name, "Value: {{ name }}").await;

    // First render
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=CACHED&name=First", name)))
        .send()
        .await
        .unwrap();
    let body1 = resp.text().await.unwrap();
    assert!(body1.contains("First"));

    // Second render with same mac_address - should return cached
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=CACHED&name=Second", name)))
        .send()
        .await
        .unwrap();
    let body2 = resp.text().await.unwrap();
    assert!(body2.contains("First"), "Expected cached 'First', got: {}", body2);

    // Different mac_address - should get new render
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=NEW&name=Third", name)))
        .send()
        .await
        .unwrap();
    let body3 = resp.text().await.unwrap();
    assert!(body3.contains("Third"));

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_dynamic_field_generation() {
    let client = Client::new();
    let name = unique_name("dynamic");

    // Create template using multipart
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Set config with dynamic fields (using new format with unified config endpoint)
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({
            "id_field": "mac_address",
            "dynamic_fields": [
                {"field_name": "password", "type": "alphanumeric", "length": 16, "hashing_algorithm": "none"}
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render - should have generated password
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=DYN:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("Password: "), "Unexpected body: {}", body);
    // Password should be 16 alphanumeric characters
    let password = body.strip_prefix("Password: ").unwrap();
    assert_eq!(password.len(), 16, "Password length should be 16: {}", password);
    assert!(password.chars().all(|c| c.is_ascii_alphanumeric()), "Password should be alphanumeric: {}", password);

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_list_and_get_rendered() {
    let client = Client::new();
    let name = unique_name("rendered");

    // Create and render a template using multipart
    upload_template(&client, &name, "Rendered test").await;

    client
        .get(url(&format!("/api/v1/template/{}?mac_address=LIST:01", name)))
        .send()
        .await
        .unwrap();

    client
        .get(url(&format!("/api/v1/template/{}?mac_address=LIST:02", name)))
        .send()
        .await
        .unwrap();

    // List rendered
    let resp = client
        .get(url(&format!("/api/v1/rendered/{}", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let data = body.as_array().unwrap();
    assert_eq!(data.len(), 2);

    // Get specific rendered
    let resp = client
        .get(url(&format!("/api/v1/rendered/{}/LIST:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["id_field_value"], "LIST:01");

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_invalid_template_rejected() {
    let client = Client::new();
    let name = unique_name("invalid");

    // Try to create template with invalid syntax using multipart
    let resp = upload_template(&client, &name, "Hello {{ name").await;

    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "error");
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_missing_template_error() {
    let client = Client::new();
    let name = unique_name("nonexistent");

    // Try to render non-existent template
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=XX", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(body.contains("not found"));
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_missing_id_field_error() {
    let client = Client::new();
    let name = unique_name("noid");

    // Create template using multipart
    upload_template(&client, &name, "Hello").await;

    // Try to render without providing mac_address
    let resp = client
        .get(url(&format!("/api/v1/template/{}", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Missing required field"));

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_custom_id_field() {
    let client = Client::new();
    let name = unique_name("customid");

    // Create template using multipart
    upload_template(&client, &name, "Serial: {{ serial_number }}").await;

    // Set custom id field using unified config endpoint
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({"id_field": "serial_number"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render with serial_number instead of mac_address
    let resp = client
        .get(url(&format!("/api/v1/template/{}?serial_number=SN123", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("SN123"));

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_invalid_yaml_values_rejected() {
    let client = Client::new();
    let name = unique_name("invalidyaml");

    // Create template
    upload_template(&client, &name, "Hello {{ name }}").await;

    // Try to set values with invalid YAML syntax
    let resp = client
        .put(url(&format!("/api/v1/template/{}/values", name)))
        .body("invalid: [yaml: missing bracket")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "error");

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_valid_json_values_accepted() {
    let client = Client::new();
    let name = unique_name("jsonvalues");

    // Create template
    upload_template(&client, &name, "Hello {{ name }}, count: {{ count }}").await;

    // Set values using JSON (which is valid YAML)
    let resp = client
        .put(url(&format!("/api/v1/template/{}/values", name)))
        .body(r#"{"name": "World", "count": 42}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render and verify JSON values work
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=JSON:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("World"), "Expected 'World' in: {}", body);
    assert!(body.contains("42"), "Expected '42' in: {}", body);

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_delete_template() {
    let client = Client::new();
    let name = unique_name("delete");

    // Create template
    let resp = upload_template(&client, &name, "Delete me").await;
    assert_eq!(resp.status(), 200);

    // Verify it exists by rendering
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=DEL:01", name)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Delete the template
    let resp = client
        .delete(url(&format!("/api/v1/template/{}", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // Verify template no longer exists
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=DEL:02", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(body.contains("not found"));
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_invalid_config_json_rejected() {
    let client = Client::new();
    let name = unique_name("invalidconfig");

    // Create template
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Try to set config with invalid JSON
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .header("Content-Type", "application/json")
        .body(r#"{"dynamic_fields": [{"field_name": "password", "type": invalid}]}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400); // Bad Request for invalid JSON

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_get_config() {
    let client = Client::new();
    let name = unique_name("getconfig");

    // Create template
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Set config
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({
            "id_field": "serial_number",
            "dynamic_fields": [
                {"field_name": "password", "type": "alphanumeric", "length": 16, "hashing_algorithm": "sha512"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Get config back
    let resp = client
        .get(url(&format!("/api/v1/config/{}", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["id_field"], "serial_number");
    assert_eq!(body["dynamic_fields"][0]["field_name"], "password");
    assert_eq!(body["dynamic_fields"][0]["type"], "alphanumeric");
    assert_eq!(body["dynamic_fields"][0]["length"], 16);
    assert_eq!(body["dynamic_fields"][0]["hashing_algorithm"], "sha512");

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_sha512_hashing() {
    let client = Client::new();
    let name = unique_name("sha512");

    // Create template
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Set config with SHA-512 hashing
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({
            "id_field": "mac_address",
            "dynamic_fields": [
                {"field_name": "password", "type": "alphanumeric", "length": 16, "hashing_algorithm": "sha512"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Render - should have hashed password with $6$ prefix
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=HASH:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("Password: $6$"), "Expected SHA-512 hash with $6$ prefix: {}", body);

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_yescrypt_hashing() {
    let client = Client::new();
    let name = unique_name("yescrypt");

    // Create template
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Set config with Yescrypt hashing
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({
            "id_field": "mac_address",
            "dynamic_fields": [
                {"field_name": "password", "type": "alphanumeric", "length": 16, "hashing_algorithm": "yescrypt"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Render - should have hashed password with $y$ prefix
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=HASH:02", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("Password: $y$"), "Expected Yescrypt hash with $y$ prefix: {}", body);

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_dynamic_field_caching() {
    let client = Client::new();
    let name = unique_name("dyncache");

    // Create template with dynamic field
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Set config with dynamic field
    client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({
            "id_field": "mac_address",
            "dynamic_fields": [
                {"field_name": "password", "type": "alphanumeric", "length": 16, "hashing_algorithm": "none"}
            ]
        }))
        .send()
        .await
        .unwrap();

    // First render - generates password
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=CACHE:01", name)))
        .send()
        .await
        .unwrap();
    let body1 = resp.text().await.unwrap();
    let password1 = body1.strip_prefix("Password: ").unwrap();

    // Second render with same ID - should return cached password
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=CACHE:01", name)))
        .send()
        .await
        .unwrap();
    let body2 = resp.text().await.unwrap();
    let password2 = body2.strip_prefix("Password: ").unwrap();

    assert_eq!(password1, password2, "Expected same cached password");

    // Different ID - should generate new password
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=CACHE:02", name)))
        .send()
        .await
        .unwrap();
    let body3 = resp.text().await.unwrap();
    let password3 = body3.strip_prefix("Password: ").unwrap();

    assert_ne!(password1, password3, "Expected different password for different ID");

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_passphrase_generation() {
    let client = Client::new();
    let name = unique_name("passphrase");

    // Create template
    upload_template(&client, &name, "Passphrase: {{ secret }}").await;

    // Set config with passphrase generator
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({
            "id_field": "mac_address",
            "dynamic_fields": [
                {"field_name": "secret", "type": "passphrase", "word_count": 4, "hashing_algorithm": "none"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Render - should have generated passphrase with 4 words separated by dashes
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=PASS:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    let passphrase = body.strip_prefix("Passphrase: ").unwrap();
    let word_count = passphrase.split('-').count();
    assert_eq!(word_count, 4, "Expected 4 words, got: {}", passphrase);

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_mixed_hashing_algorithms() {
    let client = Client::new();
    let name = unique_name("mixedhash");

    // Create template with multiple dynamic fields
    upload_template(&client, &name, "Plain: {{ plain }}\nSHA512: {{ sha_pass }}\nYescrypt: {{ yes_pass }}").await;

    // Set config with different hashing algorithms per field
    let resp = client
        .put(url(&format!("/api/v1/config/{}", name)))
        .json(&json!({
            "id_field": "mac_address",
            "dynamic_fields": [
                {"field_name": "plain", "type": "alphanumeric", "length": 12, "hashing_algorithm": "none"},
                {"field_name": "sha_pass", "type": "alphanumeric", "length": 16, "hashing_algorithm": "sha512"},
                {"field_name": "yes_pass", "type": "passphrase", "word_count": 3, "hashing_algorithm": "yescrypt"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Render - each field should have its own hashing applied
    let resp = client
        .get(url(&format!("/api/v1/template/{}?mac_address=MIX:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    let lines: Vec<&str> = body.lines().collect();

    // Plain should be 12 alphanumeric characters (no hash prefix)
    let plain = lines[0].strip_prefix("Plain: ").unwrap();
    assert_eq!(plain.len(), 12, "Plain should be 12 chars: {}", plain);
    assert!(plain.chars().all(|c| c.is_ascii_alphanumeric()), "Plain should be alphanumeric: {}", plain);
    assert!(!plain.starts_with('$'), "Plain should not be hashed: {}", plain);

    // SHA512 should have $6$ prefix
    let sha_pass = lines[1].strip_prefix("SHA512: ").unwrap();
    assert!(sha_pass.starts_with("$6$"), "SHA512 hash should have $6$ prefix: {}", sha_pass);

    // Yescrypt should have $y$ prefix
    let yes_pass = lines[2].strip_prefix("Yescrypt: ").unwrap();
    assert!(yes_pass.starts_with("$y$"), "Yescrypt hash should have $y$ prefix: {}", yes_pass);

    // Cleanup
    client.delete(url(&format!("/api/v1/template/{}", name))).send().await.unwrap();
}
