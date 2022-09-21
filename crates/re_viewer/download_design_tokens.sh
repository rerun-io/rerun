#!/usr/bin/env bash
set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../.."

curl https://rerun-design-guidelines.netlify.app/api/tokens | jq > crates/re_viewer/data/design_tokens.json

# See https://rerun-design-guidelines.netlify.app/tokens for their meanings
