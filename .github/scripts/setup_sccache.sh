#!/usr/bin/env bash
set -euo pipefail

MIRROR_BASE_URL="https://build.rerun.io/mirror/mozilla/sccache"

usage() {
  cat <<'EOF'
Install and configure sccache for CI.

Usage: setup_sccache.sh --version VERSION [--backend auto|s3|gcs] [--gcs-bucket BUCKET] [--gcs-read-only [true|false]]
EOF
}

version=""
backend="auto"
gcs_bucket="rerun-sccache"
gcs_read_only="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      version="${2:?--version requires a value}"
      shift 2
      ;;
    --backend)
      backend="${2:?--backend requires a value}"
      shift 2
      ;;
    --gcs-bucket)
      gcs_bucket="${2:?--gcs-bucket requires a value}"
      shift 2
      ;;
    --gcs-read-only)
      if [[ "${2:-}" == "true" || "${2:-}" == "false" ]]; then
        gcs_read_only="$2"
        shift 2
      else
        gcs_read_only="true"
        shift
      fi
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$version" ]]; then
  echo "--version is required" >&2
  usage >&2
  exit 2
fi

append_line() {
  local path="$1"
  local line="$2"
  printf '%s\n' "$line" >> "$path"
}

export_env() {
  local name="$1"
  local value="$2"
  export "$name=$value"
  append_line "${GITHUB_ENV:?GITHUB_ENV is required}" "$name=$value"
}

runner_arch() {
  local raw="${RUNNER_ARCH:-$(uname -m)}"
  case "$raw" in
    X64|x64|x86_64|AMD64|amd64) echo "x86_64" ;;
    ARM64|arm64|aarch64) echo "aarch64" ;;
    ARM|arm|armv7|armv7l) echo "armv7" ;;
    *) echo "Unsupported architecture: $raw" >&2; exit 1 ;;
  esac
}

runner_platform() {
  local raw="${RUNNER_OS:-$(uname -s)}"
  case "$raw" in
    Linux|linux) echo "unknown-linux-musl tar.gz sccache" ;;
    macOS|Darwin|darwin) echo "apple-darwin tar.gz sccache" ;;
    Windows|windows|MINGW*|MSYS*|CYGWIN*) echo "pc-windows-msvc zip sccache.exe" ;;
    *) echo "Unsupported OS: $raw" >&2; exit 1 ;;
  esac
}

download() {
  local url="$1"
  local destination="$2"
  local curl_args=(--fail --location --retry 4 --connect-timeout 30 --max-time 120)
  if curl --help all 2>/dev/null | grep -q -- '--retry-all-errors'; then
    curl_args+=(--retry-all-errors)
  fi
  echo "Downloading $url" >&2
  curl "${curl_args[@]}" --user-agent "rerun-setup-sccache" --output "$destination" "$url"
}

sha1_file() {
  local path="$1"
  if command -v sha1sum >/dev/null 2>&1; then
    sha1sum "$path" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 1 "$path" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl sha1 "$path" | awk '{print $NF}'
  else
    echo "No SHA1 tool found" >&2
    exit 1
  fi
}

verify_sha1() {
  local archive="$1"
  local sha1_path="$2"
  local expected actual
  expected="$(awk '{print $1}' "$sha1_path")"
  actual="$(sha1_file "$archive")"
  actual="$(printf '%s' "$actual" | tr '[:upper:]' '[:lower:]')"
  expected="$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')"
  if [[ "$actual" != "$expected" ]]; then
    echo "SHA1 mismatch for $(basename "$archive"): expected $expected, got $actual" >&2
    exit 1
  fi
}

extract_archive() {
  local archive="$1"
  local destination="$2"
  local extension="$3"
  if [[ "$extension" == "zip" ]]; then
    if command -v unzip >/dev/null 2>&1; then
      unzip -q "$archive" -d "$destination"
    elif command -v powershell.exe >/dev/null 2>&1; then
      powershell.exe -NoProfile -Command "Expand-Archive -LiteralPath '$archive' -DestinationPath '$destination' -Force"
    else
      echo "No zip extraction tool found" >&2
      exit 1
    fi
  else
    tar -xzf "$archive" -C "$destination"
  fi
}

resolve_backend() {
  if [[ "$backend" != "auto" ]]; then
    echo "$backend"
  elif [[ "${RUNNER_OS:-}" == "Linux" && -n "${RUNS_ON_S3_BUCKET_CACHE:-}" ]]; then
    echo "s3"
  else
    echo "gcs"
  fi
}

write_config() {
  local path="$1"
  local resolved_backend="$2"
  local rw_mode="READ_WRITE"

  if [[ "$gcs_read_only" == "true" ]]; then
    rw_mode="READ_ONLY"
  fi

  {
    echo "server_startup_timeout_ms = 60000"
    if [[ "$resolved_backend" == "gcs" ]]; then
      cat <<EOF

[cache.gcs]
bucket = "$gcs_bucket"
key_prefix = "_sccache"
rw_mode = "$rw_mode"
EOF
    fi
  } > "$path"
}

configure_backend() {
  local resolved_backend="$1"
  case "$resolved_backend" in
    s3)
      if [[ -z "${RUNS_ON_S3_BUCKET_CACHE:-}" ]]; then
        echo "RUNS_ON_S3_BUCKET_CACHE is required for s3 backend" >&2
        exit 1
      fi
      if [[ -z "${RUNS_ON_AWS_REGION:-}" ]]; then
        echo "RUNS_ON_AWS_REGION is required for s3 backend" >&2
        exit 1
      fi
      export_env "SCCACHE_BUCKET" "$RUNS_ON_S3_BUCKET_CACHE"
      export_env "SCCACHE_REGION" "$RUNS_ON_AWS_REGION"
      export_env "SCCACHE_S3_KEY_PREFIX" "cache/sccache"
      ;;
    gcs)
      ;;
    *)
      echo "Unsupported backend: $resolved_backend" >&2
      exit 1
      ;;
  esac

  write_config "${SCCACHE_CONF:?SCCACHE_CONF is required}" "$resolved_backend"
}

start_server() {
  local sccache_bin="$1"
  for attempt in 1 2 3; do
    echo "Starting sccache server (attempt $attempt/3)…"
    if "$sccache_bin" --start-server; then
      "$sccache_bin" --show-stats
      return
    fi
    if [[ "$attempt" != "3" ]]; then
      sleep $((attempt * 5))
    fi
  done
  echo "sccache --start-server failed after 3 attempts" >&2
  exit 1
}

install_sccache() {
  local arch target_platform extension exe filename install_root archive sha1_path extract_dir bin_dir sccache_bin base_url
  arch="$(runner_arch)"
  read -r target_platform extension exe <<< "$(runner_platform)"
  filename="sccache-${version}-${arch}-${target_platform}.${extension}"
  install_root="${RUNNER_TEMP:?RUNNER_TEMP is required}/sccache-${version}-${arch}-${target_platform}"
  archive="$install_root/$filename"
  sha1_path="$archive.sha1"
  extract_dir="$install_root/extract"
  bin_dir="$extract_dir/sccache-${version}-${arch}-${target_platform}"
  sccache_bin="$bin_dir/$exe"

  mkdir -p "$install_root" "$extract_dir"
  base_url="$MIRROR_BASE_URL/$version"
  download "$base_url/$filename" "$archive"
  download "$base_url/$filename.sha1" "$sha1_path"
  verify_sha1 "$archive" "$sha1_path"
  extract_archive "$archive" "$extract_dir" "$extension"

  if [[ ! -f "$sccache_bin" ]]; then
    echo "Missing extracted sccache binary at $sccache_bin" >&2
    echo "Extracted files:" >&2
    find "$extract_dir" -print >&2
    exit 1
  fi

  append_line "${GITHUB_PATH:?GITHUB_PATH is required}" "$bin_dir"
  echo "$sccache_bin"
}

case "$backend" in
  auto|s3|gcs) ;;
  *) echo "Unsupported backend: $backend" >&2; exit 2 ;;
esac

resolved_backend="$(resolve_backend)"
sccache_bin="$(install_sccache)"

export_env "SCCACHE_PATH" "$sccache_bin"
export_env "SCCACHE_GHA_ENABLED" "false"
export_env "RUSTC_WRAPPER" "sccache"
export_env "CARGO_INCREMENTAL" "0"
export_env "SCCACHE_CONF" "${RUNNER_TEMP:?RUNNER_TEMP is required}/sccache-config.toml"
configure_backend "$resolved_backend"
start_server "$sccache_bin"
