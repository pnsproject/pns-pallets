cargo fmt
cargo clippy
cargo nextest run
cargo build
cargo clippy --no-default-features --features runtime-benchmarks
cargo nextest run --package pns-registrar --lib --all-features -- benchmarks
cargo build --package pns-resolvers --lib --all-features
