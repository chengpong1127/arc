#!/bin/sh

# Install cudaenv from a prebuilt GitHub release, then offer to configure CUDA.
set -eu

REPOSITORY="${CUDAENV_REPOSITORY:-chengpong1127/cudaenv}"
INSTALL_DIR="${CUDAENV_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${CUDAENV_VERSION:-latest}"

say() {
    printf '%s\n' "$*"
}

fail() {
    say "error: $*" >&2
    exit 1
}

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

download() {
    url="$1"
    destination="$2"

    case "$url" in
        https://*) ;;
        *) fail "refusing non-HTTPS download URL: $url" ;;
    esac

    if command_exists curl; then
        curl --fail --location --silent --show-error --proto '=https' --tlsv1.2 "$url" --output "$destination"
    elif command_exists wget; then
        wget --quiet "$url" --output-document="$destination"
    else
        fail "curl or wget is required to download cudaenv"
    fi
}

[ "$(uname -s)" = "Linux" ] || fail "cudaenv currently supports Linux only"

case "$(uname -m)" in
    x86_64 | amd64) architecture="x86_64" ;;
    aarch64 | arm64) architecture="aarch64" ;;
    *) fail "unsupported CPU architecture: $(uname -m)" ;;
esac

target="${architecture}-unknown-linux-musl"
archive="cudaenv-${target}.tar.gz"

if [ -n "${CUDAENV_DOWNLOAD_URL:-}" ]; then
    download_url="$CUDAENV_DOWNLOAD_URL"
elif [ "$VERSION" = "latest" ]; then
    download_url="https://github.com/${REPOSITORY}/releases/latest/download/${archive}"
else
    case "$VERSION" in
        v*) tag="$VERSION" ;;
        *) tag="v$VERSION" ;;
    esac
    download_url="https://github.com/${REPOSITORY}/releases/download/${tag}/${archive}"
fi

temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT HUP INT TERM

say "Downloading cudaenv for ${target}..."
download "$download_url" "$temporary_directory/$archive" ||
    fail "could not download $download_url"
download "${CUDAENV_CHECKSUM_URL:-${download_url}.sha256}" "$temporary_directory/$archive.sha256" ||
    fail "could not download the release checksum"

expected_checksum="$(sed -n '1{s/[[:space:]].*//;p;}' "$temporary_directory/$archive.sha256")"
case "$expected_checksum" in
    *[!0-9a-fA-F]* | '') fail "release checksum is invalid" ;;
esac
if command_exists sha256sum; then
    actual_checksum="$(sha256sum "$temporary_directory/$archive" | sed 's/[[:space:]].*//')"
elif command_exists shasum; then
    actual_checksum="$(shasum -a 256 "$temporary_directory/$archive" | sed 's/[[:space:]].*//')"
else
    fail "sha256sum or shasum is required to verify cudaenv"
fi
[ "$actual_checksum" = "$expected_checksum" ] || fail "release checksum verification failed"

tar -xzf "$temporary_directory/$archive" -C "$temporary_directory"
[ -f "$temporary_directory/cudaenv" ] || fail "downloaded archive does not contain cudaenv"

mkdir -p "$INSTALL_DIR"
if command_exists install; then
    install -m 755 "$temporary_directory/cudaenv" "$INSTALL_DIR/cudaenv"
else
    cp "$temporary_directory/cudaenv" "$INSTALL_DIR/cudaenv"
    chmod 755 "$INSTALL_DIR/cudaenv"
fi

say "Installed cudaenv to $INSTALL_DIR/cudaenv"
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        say "Add $INSTALL_DIR to PATH to run cudaenv from a new shell:"
        say "  export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac

if [ -r /dev/tty ] && [ -w /dev/tty ] && (: </dev/tty) 2>/dev/null; then
    printf '\nInstall your CUDA environment now? [Y/n] ' >/dev/tty
    response=""
    IFS= read -r response </dev/tty || true
    case "$response" in
        n | N | no | No | nO | NO)
            say "Run '$INSTALL_DIR/cudaenv install' when you are ready."
            ;;
        *)
            "$INSTALL_DIR/cudaenv" install </dev/tty
            ;;
    esac
else
    say "Run '$INSTALL_DIR/cudaenv install' to configure your CUDA environment."
fi
