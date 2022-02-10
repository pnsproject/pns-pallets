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