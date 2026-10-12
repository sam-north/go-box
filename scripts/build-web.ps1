# Builds the browser client into web/ (served by go-box-server).
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
cargo build -p go-box-client --release --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Copy-Item target/wasm32-unknown-unknown/release/go-box-client.wasm web/
Write-Host "web/ is ready: run the server and open http://127.0.0.1:7878"
