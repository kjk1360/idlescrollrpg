$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$distRoot = Join-Path $repoRoot "dist"
$toolsDir = Join-Path $distRoot "tools"
$gameDir = Join-Path $distRoot "game"
$projectsDir = Join-Path $distRoot "projects"
$sampleProjectSrc = Join-Path $repoRoot "projects\sample"
$sampleProjectDst = Join-Path $projectsDir "sample"

Push-Location $repoRoot
try {
    cargo build --release -p belt_tools

    New-Item -ItemType Directory -Force -Path $toolsDir | Out-Null
    New-Item -ItemType Directory -Force -Path $gameDir | Out-Null
    New-Item -ItemType Directory -Force -Path $projectsDir | Out-Null

    Copy-Item -Force `
        -Path (Join-Path $repoRoot "target\release\belt_tools.exe") `
        -Destination (Join-Path $toolsDir "belt_tools.exe")
    Copy-Item -Force `
        -Path (Join-Path $repoRoot "target\release\belt_tools.exe") `
        -Destination (Join-Path $gameDir "idle_scroll_rpg.exe")

    New-Item -ItemType Directory -Force -Path $sampleProjectDst | Out-Null
    Copy-Item -Force -Recurse -Path (Join-Path $sampleProjectSrc "project.json") -Destination $sampleProjectDst
    Copy-Item -Force -Recurse -Path (Join-Path $sampleProjectSrc "schema") -Destination $sampleProjectDst
    Copy-Item -Force -Recurse -Path (Join-Path $sampleProjectSrc "data") -Destination $sampleProjectDst
    Copy-Item -Force -Recurse -Path (Join-Path $sampleProjectSrc "views") -Destination $sampleProjectDst
    Copy-Item -Force -Recurse -Path (Join-Path $sampleProjectSrc "build") -Destination $sampleProjectDst
    $assetsSrc = Join-Path $sampleProjectSrc "assets"
    if (Test-Path $assetsSrc) {
        Copy-Item -Force -Recurse -Path $assetsSrc -Destination $sampleProjectDst
    }

    $gameBat = Join-Path $gameDir "Run Idle Scroll RPG.bat"
    Set-Content -Encoding ASCII -Path $gameBat -Value @"
@echo off
cd /d "%~dp0"
start "" http://127.0.0.1:7880
idle_scroll_rpg.exe game --project ..\projects\sample --addr 127.0.0.1:7880
"@

    $readme = Join-Path $gameDir "README.txt"
    Set-Content -Encoding ASCII -Path $readme -Value @"
Idle Scroll RPG local game client

Run:
  Run Idle Scroll RPG.bat

Manual:
  idle_scroll_rpg.exe game --project ..\projects\sample --addr 127.0.0.1:7880
  then open http://127.0.0.1:7880

This is the game-client view, separate from the combat preview/debug view.
"@

    Write-Host "Packaged tools:"
    Write-Host "  $(Join-Path $toolsDir "belt_tools.exe")"
    Write-Host "Packaged game client:"
    Write-Host "  $(Join-Path $gameDir "idle_scroll_rpg.exe")"
    Write-Host "  $gameBat"
    Write-Host "Packaged sample project:"
    Write-Host "  $sampleProjectDst"
}
finally {
    Pop-Location
}
