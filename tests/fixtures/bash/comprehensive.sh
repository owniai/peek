#!/bin/bash
# Bash comprehensive test fixture for peek
# Covers: function (keyword + POSIX), const (readonly + declare -r), nested scope

# Top-level constants
readonly APP_VERSION="1.0.0"
readonly DEBUG_MODE=false

declare -r MAX_RETRIES=3
declare -r TIMEOUT=30

# Top-level function with function keyword
function build_project {
    echo "Building project..."
    make clean
    make all
}

# Top-level function with POSIX syntax
run_tests() {
    echo "Running tests..."
    ./run_tests.sh
}

# Function with nested function and const
function configure {
    readonly CONFIG_DIR="/etc/myapp"

    function load_config {
        echo "Loading config from $CONFIG_DIR"
        source "$CONFIG_DIR/app.conf"
    }
}

# POSIX function with nested function
deploy() {
    function validate_env {
        echo "Validating environment..."
    }

    readonly DEPLOY_TARGET="production"
    validate_env
    echo "Deploying to $DEPLOY_TARGET"
}

# Multiple readonly in one line
readonly SMTP_PORT=587 MAX_CONNECTIONS=100

# Function with declare -r inside
function setup {
    declare -r SETUP_PATH="/usr/local/bin"
    echo "Setup at $SETUP_PATH"
}

# Deeply nested function
function main {
    function init {
        readonly INIT_FLAG=true

        function finalize {
            echo "Finalizing..."
        }
    }
}

# local variables (should NOT be extracted)
function with_local {
    local temp_var="temp"
    local count=0
}

# declare without -r (should NOT be extracted)
declare NORMAL_VAR="mutable"
