#!/bin/bash
# Test fixture for declare with compound options (e.g., -rx, -rg, -rxi)

# declare -rx: readonly + export (common pattern for exported constants)
declare -rx GLOBAL_API_KEY="secret123"

# declare -r alone (should work - baseline)
declare -r SIMPLE_CONST="value"

# declare -rg: readonly + global
declare -rg GLOBAL_CONFIG="production"

# typeset -r: typeset is a synonym for declare in bash
typeset -r TYPESET_CONST="typeset_value"
