#!/usr/bin/env bash
# Publishes a client update on the VPS, from what tools/release-client.ps1
# uploaded: a tar with manifest.txt, manifest.sig and the files that changed,
# and optionally a new GravakSetup.exe. Installed as
# /usr/local/bin/gravak-publish-update.
#
#   gravak-publish-update <update.tar> [GravakSetup.exe]
#
# Every file the manifest lists is checked (size and SHA-256) against what
# the live folder will hold before anything changes; the files go in first,
# manifest.sig and manifest.txt last, so a client never reads a manifest
# naming files not there yet.
set -euo pipefail

LIVE=${GRAVAK_UPDATE_DIR:-/var/www/gravak-update}
DOWNLOADS=${GRAVAK_DOWNLOADS_DIR:-/srv/gravak/web/public/downloads}
DOWNLOADS_OWNER=${GRAVAK_DOWNLOADS_OWNER:-gravak}

tarball=${1:?usage: gravak-publish-update <update.tar> [GravakSetup.exe]}
installer=${2:-}

stage=$(mktemp -d "$(dirname "$LIVE")/.gravak-update-XXXXXX")
trap 'rm -rf "$stage"' EXIT

# No absolute paths or "..": the tar only fills the stage.
if tar -tf "$tarball" | grep -qE '(^/|(^|/)\.\.(/|$))'; then
    echo "refused: the tar has paths outside its folder" >&2
    exit 1
fi
tar -xf "$tarball" -C "$stage" --no-same-owner --no-same-permissions
for name in manifest.txt manifest.sig; do
    [[ -f "$stage/$name" ]] || { echo "refused: the tar has no $name" >&2; exit 1; }
done

# Each listed file, from the stage when uploaded, else the live folder.
bad=0
listed=0
while IFS='|' read -r path size hash; do
    listed=$((listed + 1))
    if [[ "$path" == /* || "$path" =~ (^|/)\.\.(/|$) ]]; then
        echo "refused: manifest path $path" >&2
        exit 1
    fi
    file="$stage/$path"
    [[ -f "$file" ]] || file="$LIVE/$path"
    if [[ ! -f "$file" ]]; then
        echo "missing: $path" >&2; bad=1; continue
    fi
    if [[ $(stat -c %s "$file") != "$size" ]] || [[ $(sha256sum "$file" | cut -d' ' -f1) != "$hash" ]]; then
        echo "differs from the manifest: $path" >&2; bad=1
    fi
done < <(sed -n 's/^file=//p' "$stage/manifest.txt" | tr -d '\r')
if (( bad )); then
    echo "refused: nothing was published" >&2
    exit 1
fi

mkdir -p "$LIVE"
# Keep the manifest being replaced, to go back to by hand.
if [[ -f "$LIVE/manifest.txt" ]]; then
    cp -p "$LIVE/manifest.txt" "$LIVE/manifest.sig" "$(dirname "$LIVE")/" 2>/dev/null || true
fi

# The files, each renamed into place (a download in progress keeps the old one).
copied=0
while IFS= read -r -d '' file; do
    path=${file#"$stage"/}
    [[ "$path" == manifest.txt || "$path" == manifest.sig ]] && continue
    mkdir -p "$LIVE/$(dirname "$path")"
    install -m 644 "$file" "$LIVE/$path.new"
    mv -f "$LIVE/$path.new" "$LIVE/$path"
    copied=$((copied + 1))
done < <(find "$stage" -type f -print0)
for name in manifest.sig manifest.txt; do
    install -m 644 "$stage/$name" "$LIVE/$name.new"
    mv -f "$LIVE/$name.new" "$LIVE/$name"
done
version=$(sed -n 's/^version=//p' "$LIVE/manifest.txt" | tr -d '\r')
echo "update v$version published: $copied files uploaded, $listed in the manifest"

if [[ -n "$installer" ]]; then
    install -o "$DOWNLOADS_OWNER" -g "$DOWNLOADS_OWNER" -m 644 "$installer" "$DOWNLOADS/GravakSetup.exe.new"
    mv -f "$DOWNLOADS/GravakSetup.exe.new" "$DOWNLOADS/GravakSetup.exe"
    rm -f "$installer"
    echo "installer published: $(sha256sum "$DOWNLOADS/GravakSetup.exe" | cut -d' ' -f1)"
fi
rm -f "$tarball"
