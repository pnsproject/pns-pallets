### How to run benchmarking tests:

pns-registrar:

```shell
cargo test --package pns-registrar --lib --all-features -- benchmarks
```

pns-resolvers:

```shell 
cargo build --package pns-resolvers --lib --all-features
```

cargo build:

```shell
cargo build --no-default-features --features runtime-benchmarks 
```

cargo clippy:

```shell
cargo clippy --no-default-features --features runtime-benchmarks
```

### Q&A

- Q:
```shell
pns-pallets on  main [!?] via 🦀 v1.60.0-nightly 
❯ ./before_commit.sh
zsh: permission denied: ./before_commit.sh
```
- A:
```shell
chmod u+x before_commit.sh
```

### Documents

```shell
cargo doc --open
```