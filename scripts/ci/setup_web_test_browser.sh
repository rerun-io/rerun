#!/usr/bin/env bash
set -euo pipefail

if [[ "${RUNNER_OS:-}" != "Linux" || "${RUNNER_ARCH:-}" != "X64" ]]; then
  echo "Unsupported runner: ${RUNNER_OS:-unknown}/${RUNNER_ARCH:-unknown}" >&2
  exit 1
fi

# When bumping Firefox, update its digest from the release's SHA256SUMS file.
readonly FIREFOX_VERSION="153.0.3"
readonly FIREFOX_SHA256="22b312280900bfb174b685ece32c7b3c6d72e7f8e53d6d30f21ac41a8dc500a2"

# When bumping geckodriver, download the release archive and update its digest.
readonly GECKODRIVER_VERSION="0.37.1"
readonly GECKODRIVER_SHA256="e815130ea95983e162ae91843b48d3a3ce991735635fce83a647afde21e09f7e"

readonly firefox_dir="${RUNNER_TEMP}/firefox-bin"
readonly firefox_archive="${RUNNER_TEMP}/firefox.tar.xz"
mkdir -p "${firefox_dir}"
curl --fail --location --silent --show-error \
  "https://ftp.mozilla.org/pub/firefox/releases/${FIREFOX_VERSION}/linux-x86_64/en-US/firefox-${FIREFOX_VERSION}.tar.xz" \
  --output "${firefox_archive}"
echo "${FIREFOX_SHA256}  ${firefox_archive}" | sha256sum --check
tar --extract --xz --file "${firefox_archive}" --directory "${firefox_dir}" --strip-components=1
rm "${firefox_archive}"

readonly geckodriver_dir="${RUNNER_TEMP}/geckodriver-bin"
readonly geckodriver_archive="${RUNNER_TEMP}/geckodriver.tar.gz"
mkdir -p "${geckodriver_dir}"
curl --fail --location --silent --show-error \
  "https://github.com/mozilla/geckodriver/releases/download/v${GECKODRIVER_VERSION}/geckodriver-v${GECKODRIVER_VERSION}-linux64.tar.gz" \
  --output "${geckodriver_archive}"
echo "${GECKODRIVER_SHA256}  ${geckodriver_archive}" | sha256sum --check
tar --extract --gzip --file "${geckodriver_archive}" --directory "${geckodriver_dir}"
rm "${geckodriver_archive}"

printf '%s\n' "${firefox_dir}" "${geckodriver_dir}" >> "${GITHUB_PATH}"
"${firefox_dir}/firefox" --version
"${geckodriver_dir}/geckodriver" --version
