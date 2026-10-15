# Builds the browser spike (docs/web-spike.md) into client\dist\web: the
# client compiled to wasm32, its wasm-bindgen glue, client\web\index.html and
# the few game files the login screens need (client\web\assets.txt), copied
# out of the game data. Serve the folder over HTTP (the "web-spike" entry of
# .claude\launch.json) and open it in a browser.
#
#   tools\build-web.ps1                      game data from dist\Gravak\game
#   tools\build-web.ps1 -Game R:\5065        or an installation
#   tools\build-web.ps1 -KeyFile <pack.key>  the release pack key, when it is
#                                            not installer\pack.key
#   tools\build-web.ps1 -Servers <servers.ini>  the worlds offered, with their
#                                            login keys; without it, the
#                                            server on the bridge's machine
#
# The page reaches the servers through gravak-ws-bridge: at /ws on its own
# host, or where ?bridge=ws://host:port says (local tests:
# http://localhost:8099/?bridge=ws://127.0.0.1:9960).
#
# Needs: rustup target add wasm32-unknown-unknown, and wasm-bindgen-cli at
# the version of the wasm-bindgen crate in Cargo.lock:
#   cargo install wasm-bindgen-cli --version 0.2.128 --locked
param(
    [string]$Game = "",
    [string]$KeyFile = "",
    [string]$Servers = ""
)
$ErrorActionPreference = "Stop"
$client = Split-Path -Parent $PSScriptRoot
if (-not $Game) { $Game = Join-Path $client "dist\Gravak\game" }
$out = Join-Path $client "dist\web"
# A target folder outside .claude\worktrees: MSVC's cl.exe fails there
# (D8050) when ring's C code is compiled for the exporter.
$targets = if ($env:GRAVAK_WEB_TARGET) { $env:GRAVAK_WEB_TARGET } else { Join-Path $client "target\web" }

Push-Location $client
try {
    Write-Host "Building the client for wasm32 (release)..." -ForegroundColor Cyan
    $env:CARGO_TARGET_DIR = $targets
    cargo build --release -p conquer-sandbox --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw "cargo build (wasm32) failed" }

    New-Item -ItemType Directory -Force $out | Out-Null
    wasm-bindgen --target web --no-typescript --out-name gravak --out-dir $out `
        (Join-Path $targets "wasm32-unknown-unknown\release\conquer-sandbox.wasm")
    if ($LASTEXITCODE -ne 0) { throw "wasm-bindgen failed" }
    Copy-Item (Join-Path $client "web\index.html") $out -Force
    $serversCopy = Join-Path $out "servers.ini"
    if ($Servers) { Copy-Item $Servers $serversCopy -Force }
    elseif (Test-Path $serversCopy) { Remove-Item $serversCopy }

    # The game files, copied out by the native client. A pack made with the
    # release key (installer\pack.key) only opens in a client built with it.
    Write-Host "Copying the login screens' game files..." -ForegroundColor Cyan
    if (-not $KeyFile) { $KeyFile = Join-Path $client "installer\pack.key" }
    if (Test-Path $KeyFile) { $env:GRAVAK_PACK_KEY = (Get-Content -Raw $KeyFile).Trim() }
    cargo build --release -p conquer-sandbox
    if ($LASTEXITCODE -ne 0) { throw "cargo build (exporter) failed" }
    $names = @(Get-Content (Join-Path $client "web\assets.txt") | Where-Object { $_.Trim() -and -not $_.StartsWith("#") })
    $assets = Join-Path $out "assets"
    New-Item -ItemType Directory -Force $assets | Out-Null
    & (Join-Path $targets "release\conquer-sandbox.exe") --export-assets $Game $assets @names
    if ($LASTEXITCODE -ne 0) { throw "the game files could not be copied out of $Game" }
    $names | Set-Content -Encoding ascii (Join-Path $assets "manifest.txt")
    # The preload lists: what entering any world reads (web/world.txt) and
    # each body's files (web/bodyN.txt).
    foreach ($list in @("world") + (1..4 | ForEach-Object { "body$_" })) {
        Copy-Item (Join-Path $client "web\$list.txt") (Join-Path $assets "$list.txt") -Force
    }

    $wasm = Get-Item (Join-Path $out "gravak_bg.wasm")
    Write-Host ("Done: {0} ({1:N1} MiB of wasm)" -f $out, ($wasm.Length / 1MB)) -ForegroundColor Green
}
finally {
    Remove-Item Env:CARGO_TARGET_DIR, Env:GRAVAK_PACK_KEY -ErrorAction SilentlyContinue
    Pop-Location
}
