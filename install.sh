#!/bin/sh

# Install arc from a prebuilt GitHub release, then offer to configure CUDA.
set -eu

REPOSITORY="${ARC_REPOSITORY:-chengpong1127/arc}"
INSTALL_DIR="${ARC_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${ARC_VERSION:-latest}"

if [ -t 1 ] && [ "${TERM:-dumb}" != "dumb" ] && [ -z "${NO_COLOR:-}" ]; then
    bold="$(printf '\033[1m')"
    dim="$(printf '\033[2m')"
    red="$(printf '\033[31m')"
    green="$(printf '\033[32m')"
    yellow="$(printf '\033[33m')"
    cyan="$(printf '\033[36m')"
    reset="$(printf '\033[0m')"
else
    bold=""
    dim=""
    red=""
    green=""
    yellow=""
    cyan=""
    reset=""
fi

heading() {
    printf '\n  %s%sarc%s  %sinstaller%s\n' "$bold" "$cyan" "$reset" "$dim" "$reset"
    printf '  %s\n\n' "${dim}────────────────────────────────────────${reset}"
}

step() {
    printf '  %s◆%s  %s\n' "$cyan" "$reset" "$*"
}

success() {
    printf '  %s✓%s  %s\n' "$green" "$reset" "$*"
}

note() {
    printf '  %s!%s  %s\n' "$yellow" "$reset" "$*"
}

fail() {
    printf '\n  %s✗%s  %s\n\n' "$red" "$reset" "$*" >&2
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
        fail "curl or wget is required to download arc"
    fi
}

[ "$(uname -s)" = "Linux" ] || fail "arc currently supports Linux only"

case "$(uname -m)" in
    x86_64 | amd64) architecture="x86_64" ;;
    aarch64 | arm64) architecture="aarch64" ;;
    *) fail "unsupported CPU architecture: $(uname -m)" ;;
esac

target="${architecture}-unknown-linux-musl"
archive="arc-${target}.tar.gz"

heading
step "Detected Linux / ${architecture}"

if [ -n "${ARC_DOWNLOAD_URL:-}" ]; then
    download_url="$ARC_DOWNLOAD_URL"
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

step "Downloading arc for ${target}"
download "$download_url" "$temporary_directory/$archive" ||
    fail "could not download $download_url"
download "${ARC_CHECKSUM_URL:-${download_url}.sha256}" "$temporary_directory/$archive.sha256" ||
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
    fail "sha256sum or shasum is required to verify arc"
fi
[ "$actual_checksum" = "$expected_checksum" ] || fail "release checksum verification failed"
success "Release checksum verified"

tar -xzf "$temporary_directory/$archive" -C "$temporary_directory"
[ -f "$temporary_directory/arc" ] || fail "downloaded archive does not contain arc"

mkdir -p "$INSTALL_DIR"
if command_exists install; then
    install -m 755 "$temporary_directory/arc" "$INSTALL_DIR/arc"
else
    cp "$temporary_directory/arc" "$INSTALL_DIR/arc"
    chmod 755 "$INSTALL_DIR/arc"
fi

printf '\n'
success "arc installed"
printf '     %s%s%s\n' "$dim" "$INSTALL_DIR/arc" "$reset"
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        printf '\n'
        note "Add ${INSTALL_DIR} to PATH to use arc in a new shell"
        printf '     %sexport PATH="%s:%s"%s\n' "$dim" "$INSTALL_DIR" "\$PATH" "$reset"
        ;;
esac

if [ -r /dev/tty ] && [ -w /dev/tty ] && (: </dev/tty) 2>/dev/null; then
    printf '\n  %s◆%s  %sInstall your CUDA environment now?%s %s[Y/n]%s ' \
        "$cyan" "$reset" "$bold" "$reset" "$dim" "$reset" >/dev/tty
    response=""
    IFS= read -r response </dev/tty || true
    case "$response" in
        n | N | no | No | nO | NO)
            printf '\n'
            note "Run '$INSTALL_DIR/arc install' when you are ready"
            ;;
        *)
            "$INSTALL_DIR/arc" install </dev/tty
            ;;
    esac
else
    printf '\n'
    note "Run '$INSTALL_DIR/arc install' to configure your CUDA environment"
fi
