#!/usr/bin/env bash
export DATABASE_PATH="/Users/$USER/Library/Application Support/am-osx-status/sqlite.db"

if [ "$1" = "restore" ]; then
    if [ ! -f "$DATABASE_PATH.bak" ]; then
        echo "Backup not found at $DATABASE_PATH.bak"
        exit 1
    fi
    mv "$DATABASE_PATH" "$DATABASE_PATH.tmp"
    mv "$DATABASE_PATH.bak" "$DATABASE_PATH"
    mv "$DATABASE_PATH.tmp" "$DATABASE_PATH.bak"
    rm "$DATABASE_PATH.tmp"
    echo "Swapped to backup at $DATABASE_PATH.bak; backup is now previous state"
else
    if [ ! -f "$DATABASE_PATH" ]; then
        echo "Database not found at $DATABASE_PATH"
        exit 1
    fi
    if [ -f "$DATABASE_PATH.bak" ]; then
        if [ "$1" = "force" ]; then
            rm "$DATABASE_PATH.bak"
        else
            echo "Backup already exists at $DATABASE_PATH.bak"
            exit 1
        fi
    fi

    cp "$DATABASE_PATH" "$DATABASE_PATH.bak"
    echo "Backup created at $DATABASE_PATH.bak"
fi
