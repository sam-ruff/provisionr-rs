# Provisionr

A REST-based template provisioning system for generating configuration files with dynamic values. Uses Jinja2 templates with automatic caching by a configurable ID field.

## Running

```bash
# Default port 3000
cargo run --release

# Custom port and database
PROVISIONR_PORT=8080 PROVISIONR_DB=./data.db cargo run --release
```

Swagger UI available at `http://localhost:3000/swagger-ui/`

## Testing

```bash
# Unit tests only (no server required)
cargo test

# Integration tests (requires running server)
cargo run --release &
cargo test --ignored

# All tests
cargo test --include-ignored
```

## API

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/template/{name}` | Upload template (multipart file) |
| GET | `/api/template/{name}` | Render template with query params |
| DELETE | `/api/template/{name}` | Delete template |
| PUT | `/api/template/{name}/values` | Set default values (YAML body) |
| PUT | `/api/template/{name}/id-field` | Set caching ID field |
| PUT | `/api/template/{name}/dynamic-fields` | Configure generated fields |
| GET | `/api/rendered/{name}` | List cached renders |
| GET | `/api/rendered/{name}/{id}` | Get specific cached render |

Template names automatically get `.j2` extension appended if missing.

## Building

```bash
# Debug build
cargo build

# Release build (optimised)
cargo build --release

# Binary location
./target/release/provisionr
```

## Cross-compilation

Statically-linked musl builds for ARM:

```bash
# Install cross (handles toolchains via Docker)
cargo install cross --git https://github.com/cross-rs/cross

# Build for ARM64 (Raspberry Pi 4, etc.)
cross build --release --target aarch64-unknown-linux-musl

# Build for ARMv7 (older Pi, Zynq)
cross build --release --target armv7-unknown-linux-musleabihf

# Build for x86_64 Linux
cross build --release --target x86_64-unknown-linux-musl
```
