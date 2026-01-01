# Provisionr

A REST-based template provisioning system for generating configuration files with dynamic values. Uses Jinja2 templates with automatic caching by a configurable ID field.

## Running

```bash
# Default port 3000
cargo run --release

# Custom port and database
cargo run --release -- --port 8080 --db ./data.db

# With debug logging
cargo run --release -- --log-level debug

# With config file
cargo run --release -- --config config.yaml
```

CLI options:
- `--config`, `-c`: Path to YAML configuration file
- `--port`, `-p`: Port to listen on (default: 3000)
- `--db`: Database path (default: provisionr.db)
- `--log-level`: Log level - trace, debug, info, warn, error (default: info)

CLI arguments override config file values.

Swagger UI available at `http://localhost:3000/swagger-ui/`

## Configuration

See `config.example.yaml` for a complete example. Copy it to `config.yaml` and modify as needed:

```yaml
log_level: info
port: 3000
db: provisionr.db
```

## Testing

```bash
# Unit tests only (no server required)
cargo test

# Integration tests (requires running server)
cargo run --release &
cargo test -- --ignored

# All tests
cargo test -- --include-ignored
```

## API

### Templates

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/template/{name}` | Upload template (multipart file) |
| GET | `/api/v1/template/{name}` | Render template with query params |
| DELETE | `/api/v1/template/{name}` | Delete template |
| PUT | `/api/v1/template/{name}/values` | Set default values (YAML/JSON body) |

### Configuration

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/config/{name}` | Get template configuration |
| PUT | `/api/v1/config/{name}` | Set template configuration |

Configuration includes:
- `id_field`: Query parameter used for caching (default: mac_address)
- `dynamic_fields`: Auto-generated values (alphanumeric or passphrase)
- `hashing_algorithm`: none, sha512, or yescrypt

### Rendered Templates

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/rendered/{name}` | List cached renders |
| GET | `/api/v1/rendered/{name}/{id}` | Get specific cached render |

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
