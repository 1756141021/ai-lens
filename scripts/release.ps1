#Requires -Version 7
# Build, sign, generate latest.json, and publish a GitHub release.
# Needs: .signing/ai-lens.key + .signing/key-password.txt (gitignored), gh CLI logged in.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$repo = "1756141021/ai-lens"

$conf = Get-Content "$root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
$version = $conf.version
$tag = "v$version"

$existing = $null
try { $existing = gh release view $tag --repo $repo --json tagName 2>$null } catch {}
if ($existing) { throw "release $tag already exists" }

$keyFile = "$root\.signing\ai-lens.key"
$pwFile = "$root\.signing\key-password.txt"
if (-not (Test-Path $keyFile) -or -not (Test-Path $pwFile)) {
  throw "missing .signing/ai-lens.key or key-password.txt"
}
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $keyFile -Raw)
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content $pwFile -Raw)

Set-Location $root
pnpm tauri build
if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

$bundle = "$root\src-tauri\target\release\bundle"
$setup = Get-Item "$bundle\nsis\AI Lens_${version}_x64-setup.exe"
$sigFile = "$($setup.FullName).sig"
$msi = Get-Item "$bundle\msi\AI Lens_${version}_x64_en-US.msi"
if (-not (Test-Path $sigFile)) {
  throw ".sig missing — signing env vars or bundle.createUpdaterArtifacts broken"
}

# GitHub replaces spaces in asset filenames with dots
$assetName = $setup.Name -replace " ", "."
$latest = [ordered]@{
  version = $version
  notes = "https://github.com/$repo/releases/tag/$tag"
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss'Z'")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{
      signature = (Get-Content $sigFile -Raw).Trim()
      url = "https://github.com/$repo/releases/download/$tag/$assetName"
    }
  }
}
$latestPath = "$bundle\latest.json"
$latest | ConvertTo-Json -Depth 5 | Set-Content $latestPath

gh release create $tag $setup.FullName $sigFile $msi.FullName $latestPath `
  --repo $repo --title "AI Lens $tag" --generate-notes
if ($LASTEXITCODE -ne 0) { throw "gh release create failed" }
Write-Host "released $tag — check https://github.com/$repo/releases/latest/download/latest.json"
