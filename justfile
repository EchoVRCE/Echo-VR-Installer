# Keep Rust builds bounded on memory-constrained developer machines.
# Override with `CARGO_BUILD_JOBS=1 just check` when the machine is under heavier load.
cargo_jobs := env_var_or_default("CARGO_BUILD_JOBS", "2")

# List available project checks.
default:
    @just --list

# Check formatting without compiling the project.
fmt:
    cargo fmt --check

# Run tests with the shared Cargo job cap.
test *args:
    cargo test -j {{cargo_jobs}} {{args}}

# Run the same strict lint gate as CI, with bounded compilation.
clippy:
    cargo clippy -j {{cargo_jobs}} --all-targets -- -D warnings

# Build the optimized launcher with bounded compilation.
release:
    cargo build -j {{cargo_jobs}} --release

# Run verification sequentially so multiple Cargo builds never compete for memory.
check:
    just fmt
    just clippy
    just test
    just release
