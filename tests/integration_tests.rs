use reqwest::multipart;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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
        .post(url(&format!("/api/template/{}", name)))
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

    // Delete template (stored name has .j2 appended)
    let resp = client
        .delete(url(&format!("/api/template/{}.j2", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // Verify template is gone
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=XX", name)))
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
        .put(url(&format!("/api/template/{}.j2/values", name)))
        .body("name: World\nage: 42")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render and verify values are used
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=AA:BB:CC", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("World"), "Expected 'World' in: {}", body);
    assert!(body.contains("42"), "Expected '42' in: {}", body);

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
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
        .get(url(&format!("/api/template/{}.j2?mac_address=AA:BB:CC&name=Integration", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "Hello Integration!");

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
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
        .get(url(&format!("/api/template/{}.j2?mac_address=CACHED&name=First", name)))
        .send()
        .await
        .unwrap();
    let body1 = resp.text().await.unwrap();
    assert!(body1.contains("First"));

    // Second render with same mac_address - should return cached
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=CACHED&name=Second", name)))
        .send()
        .await
        .unwrap();
    let body2 = resp.text().await.unwrap();
    assert!(body2.contains("First"), "Expected cached 'First', got: {}", body2);

    // Different mac_address - should get new render
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=NEW&name=Third", name)))
        .send()
        .await
        .unwrap();
    let body3 = resp.text().await.unwrap();
    assert!(body3.contains("Third"));

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_dynamic_field_generation() {
    let client = Client::new();
    let name = unique_name("dynamic");

    // Create template using multipart
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Set dynamic fields
    let resp = client
        .put(url(&format!("/api/template/{}.j2/dynamic-fields", name)))
        .json(&json!({
            "fields": [
                {"field_name": "password", "generator_type": {"Alphanumeric": 16}}
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render - should have generated password
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=DYN:01", name)))
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
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_list_and_get_rendered() {
    let client = Client::new();
    let name = unique_name("rendered");

    // Create and render a template using multipart
    upload_template(&client, &name, "Rendered test").await;

    client
        .get(url(&format!("/api/template/{}.j2?mac_address=LIST:01", name)))
        .send()
        .await
        .unwrap();

    client
        .get(url(&format!("/api/template/{}.j2?mac_address=LIST:02", name)))
        .send()
        .await
        .unwrap();

    // List rendered
    let resp = client
        .get(url(&format!("/api/rendered/{}.j2", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);

    // Get specific rendered
    let resp = client
        .get(url(&format!("/api/rendered/{}.j2/LIST:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["id_field_value"], "LIST:01");

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
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
async fn test_template_auto_appends_j2_extension() {
    let client = Client::new();
    let name = unique_name("autoext");

    // Create template without .j2 extension - it should be auto-appended
    let resp = upload_template(&client, &name, "Hello").await;
    assert_eq!(resp.status(), 200);

    // Verify template exists with .j2 extension
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=XX", name)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_missing_template_error() {
    let client = Client::new();
    let name = unique_name("nonexistent");

    // Try to render non-existent template
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=XX", name)))
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
        .get(url(&format!("/api/template/{}.j2", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Missing required field"));

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_custom_id_field() {
    let client = Client::new();
    let name = unique_name("customid");

    // Create template using multipart
    upload_template(&client, &name, "Serial: {{ serial_number }}").await;

    // Set custom id field
    let resp = client
        .put(url(&format!("/api/template/{}.j2/id-field", name)))
        .json(&json!({"id_field": "serial_number"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render with serial_number instead of mac_address
    let resp = client
        .get(url(&format!("/api/template/{}.j2?serial_number=SN123", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("SN123"));

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
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
        .put(url(&format!("/api/template/{}.j2/values", name)))
        .body("invalid: [yaml: missing bracket")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "error");

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
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
        .put(url(&format!("/api/template/{}.j2/values", name)))
        .body(r#"{"name": "World", "count": 42}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Render and verify JSON values work
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=JSON:01", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("World"), "Expected 'World' in: {}", body);
    assert!(body.contains("42"), "Expected '42' in: {}", body);

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
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
        .get(url(&format!("/api/template/{}.j2?mac_address=DEL:01", name)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Delete the template
    let resp = client
        .delete(url(&format!("/api/template/{}.j2", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // Verify template no longer exists
    let resp = client
        .get(url(&format!("/api/template/{}.j2?mac_address=DEL:02", name)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(body.contains("not found"));
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_invalid_dynamic_fields_json_rejected() {
    let client = Client::new();
    let name = unique_name("invaliddynamic");

    // Create template
    upload_template(&client, &name, "Password: {{ password }}").await;

    // Try to set dynamic fields with invalid JSON
    let resp = client
        .put(url(&format!("/api/template/{}.j2/dynamic-fields", name)))
        .header("Content-Type", "application/json")
        .body(r#"{"fields": [{"field_name": "password", "generator_type": invalid}]}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400); // Bad Request for invalid JSON

    // Cleanup
    client.delete(url(&format!("/api/template/{}.j2", name))).send().await.unwrap();
}
