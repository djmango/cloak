# Setup

## Install dependencies

Rust, Shuttle.rs and all dependencies are required to build the project.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

curl -sSfL https://www.shuttle.rs/install | bash
cargo install
```

## DB

Setup a postgres database and create a `Secrets.toml` file in the root of the project with contents of `Secrets.dev.toml`. Then migrate.

```bash
sqlx run migrate
```

## Run

```bash
cargo shuttle run
```

or with hot reload

```bash
bash scripts/watch.sh
```

## Test

Tests have not yet been implemented. SQLX checks run at compile time.

```bash
cargo check
cargo test
```
