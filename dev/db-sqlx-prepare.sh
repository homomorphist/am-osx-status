if ! cargo sqlx --version &>/dev/null; then  
    cargo install sqlx-cli --no-default-features --features sqlite-unbundled
fi

export DATABASE_URL="sqlite:///Users/$USER/Library/Application Support/am-osx-status/sqlite.db"
cargo sqlx database create
cargo sqlx prepare
