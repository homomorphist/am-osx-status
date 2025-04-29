#!/usr/bin/env bash
cargo doc --document-private-items --no-deps -p am-osx-status --workspace $1
