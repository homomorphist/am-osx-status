#!/usr/bin/env bash
if ! cargo sqlx --version &>/dev/null; then  
    cargo install sqlx-cli --no-default-features --features sqlite-unbundled
fi

export DATABASE_PATH="/Users/$USER/Library/Application Support/am-osx-status/sqlite.db"
export DATABASE_URL="sqlite://$DATABASE_PATH"

if [ "$1" = "-r" ]; then
    rm "$DATABASE_PATH"
fi

cargo sqlx database create

DIR_SELF="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
MIGRATIONS="$DIR_SELF/../src/store/sql/migrations"

if [ "$1" = "-s" ] || [ "$2" = "-s" ]; then
    echo "Skipping migrations"
    exit 0
else
    for dir in $(ls $MIGRATIONS | sed 's|/$||' | awk -F':' '{print $1, $0}' | sort -k1,1 | awk '{print $2}'); do
        sqlite3 "$DATABASE_PATH" < $MIGRATIONS/$dir/up.sql
        echo "Applied migration $dir"
    done
fi

