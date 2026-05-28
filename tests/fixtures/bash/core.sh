#!/bin/bash
# core.sh — all kind classifications, scope paths, signature formats, nested scope
# Each construct appears once; no duplicate coverage across core and edge.

# ── Function (function keyword) ──
function build_project {
    echo "Building"
}

# ── Function (POSIX definition) ──
run_tests() {
    echo "Testing"
}

# ── Const (readonly) ──
readonly APP_VERSION="1.0"

# ── Const (declare -r) ──
declare -r MAX_RETRIES=3

# ── Const (multi-readonly on one line) ──
readonly SMTP_PORT=587 MAX_CONNECTIONS=100

# ── Var (top-level assignment) ──
config_path="/etc/myapp"
timeout_seconds=30

# ── Nested function scope with nested Const ──
function configure {
    readonly CONFIG_DIR="/etc/myapp"

    function load_config {
        echo "Loading"
    }
}

# ── Function with declare -r Const inside ──
function setup {
    declare -r SETUP_PATH="/usr/local/bin"
}