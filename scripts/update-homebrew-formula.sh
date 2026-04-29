#!/bin/bash
set -euo pipefail

VERSION=$1
ARM64_MAC=$2
INTEL_MAC=$3
ARM64_LINUX=$4
INTEL_LINUX=$5

mkdir -p Formula
ruby -r erb -e '
  version, arm64_mac, intel_mac, arm64_linux, intel_linux = ARGV
  template = File.read("scripts/peek-formula.rb.erb")
  puts ERB.new(template, trim_mode: "-").result(binding)
' "$VERSION" "$ARM64_MAC" "$INTEL_MAC" "$ARM64_LINUX" "$INTEL_LINUX" > Formula/peek.rb
