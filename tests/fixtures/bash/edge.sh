#!/bin/bash
# edge.sh — boundary behaviors: compound declare options, local/typeset -r as Const,
# function-body Var exclusion, non-readonly declare exclusion, deeply nested scope,
# readonly in function scope
# Negative cases (declare without -r, local without -r, typeset without -r,
# Var in function body) are present but NOT extracted.

# ── declare -rx as Const (readonly + export) ──
declare -rx GLOBAL_API_KEY="secret123"

# ── declare -rg as Const (readonly + global) ──
declare -rg GLOBAL_CONFIG="production"

# ── declare -r as Const (baseline) ──
declare -r SIMPLE_CONST="value"

# ── typeset -r as Const ──
typeset -r TYPESET_CONST="typeset_value"

# ── local -r in function as Const ──
function setup_env {
    local -r LOCAL_CONST="fixed"
}

# ── declare without -r NOT extracted ──
declare NORMAL_VAR="mutable"

# ── local without -r NOT extracted ──
function with_local {
    local temp_var="temp"
}

# ── typeset without -r NOT extracted ──
typeset MUTABLE_VAR="changeable"

# ── Var in function body NOT extracted; nested function and readonly ARE extracted ──
deploy() {
    function validate_env {
        echo "Validating environment..."
    }

    readonly DEPLOY_TARGET="production"
    deploy_target="mutable"
}

# ── Deeply nested scope with nested Const ──
function main {
    function init {
        readonly INIT_FLAG=true

        function finalize {
            echo "Finalizing"
        }
    }
}

# ── readonly in function scope ──
function configure_app {
    readonly CONFIG_DIR="/etc/app"
}