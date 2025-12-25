# Provisionr

A REST-based template provisioning system for generating configuration files with dynamic values.

## Running Tests

### Unit Tests
```bash
cargo test --bin provisionr
```

### Integration Tests

Integration tests require a running server:

```bash
# Terminal 1: Start the server
cargo run

# Terminal 2: Run integration tests
cargo test --test integration_tests

# Or specify a custom server URL
PROVISIONR_URL=http://localhost:8080 cargo test --test integration_tests
```

## Development

```bash
# Run the server (default port 3000)
cargo run

# Run with custom port and database
PROVISIONR_PORT=8080 PROVISIONR_DB=./data.db cargo run

# Check for warnings
cargo clippy
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/template/{name}` | Create/update template (.j2 extension required) |
| GET | `/api/template/{name}` | Render template with query parameters |
| DELETE | `/api/template/{name}` | Delete a template |
| PUT | `/api/template/{name}/values` | Set YAML default values |
| PUT | `/api/template/{name}/id-field` | Set ID field for caching |
| PUT | `/api/template/{name}/dynamic-fields` | Configure dynamic value generation |
| GET | `/api/rendered/{name}` | List rendered instances |
| GET | `/api/rendered/{name}/{id_value}` | Get specific rendered template |

### Setup for ARM
Do 
```bash
rustup target add aarch64-unknown-linux-gnu # 64-bit ARM Linux target, this is dynamically linked
rustup target add aarch64-unknown-linux-musl # 64-bit ARM Linux, statically linked
```
Add `.cargo/config.toml` and specify which linkers to use for ARM targets.

Then install the toolchains for compilation:
```bash
sudo apt install gcc-aarch64-linux-gnu
# Also install the emulators
# Install system emulator
sudo apt install qemu-system-arm
# Run a bare-metal binary
qemu-system-arm -cpu cortex-m4 -machine lm3s6965evb -nographic -kernel your_binary.elf
```
Then build for the target:
```bash
cargo build --release --target aarch64-unknown-linux-musl
```

For 32 bit Arm running Zynq SoC
```bash
rustup target add armv7-unknown-linux-musleabihf
cargo build --release --target armv7-unknown-linux-musleabihf
```