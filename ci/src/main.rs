use xshell::{cmd, Shell};

fn main() {
    let sh = Shell::new().expect("Shell create field.");
    // When run locally, results may differ from actual CI runs triggered by
    // .github/workflows/ci.yml
    // - Official CI runs latest stable
    // - Local runs use whatever the default Rust is locally

    // See if any code needs to be formatted
    cmd!(sh, "cargo fmt --all -- --check")
        .run()
        .expect("Please run 'cargo fmt --all' to format your code.");

    // See if clippy has any complaints.
    // - Type complexity must be ignored because we use huge templates for queries
    cmd!(sh,"cargo clippy --workspace --all-targets --all-features -- -D warnings -A clippy::type_complexity -W clippy::doc_markdown")
        .run()
        .expect("Please fix clippy errors in output above.");

    cmd!(
        sh,
        "cargo clippy --no-default-features --features runtime-benchmarks"
    )
    .run()
    .expect("Please fix clippy errors in output above.");

    // Cargo check
    cmd!(
        sh,
        "cargo check --package pns-resolvers --lib --all-features"
    )
    .run()
    .expect("Please fix check errors in output above.");

    // These tests are already run on the CI
    // Using a double-negative here allows end-users to have a nicer experience
    // as we can pass in the extra argument to the CI script
    let args: Vec<String> = std::env::args().collect();
    if args.get(1) != Some(&"nonlocal".to_string()) {
        // Run tests
        cmd!(sh, "cargo test --workspace")
            .run()
            .expect("Please fix failing tests in output above.");

        // Run doc tests: these are ignored by `cargo test`
        cmd!(sh, "cargo test --doc --workspace")
            .run()
            .expect("Please fix failing doc-tests in output above.");
    }
}
