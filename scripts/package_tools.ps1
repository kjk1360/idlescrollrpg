$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$distRoot = Join-Path $repoRoot "dist"
$toolsDir = Join-Path $distRoot "tools"
$projectsDir = Join-Path $distRoot "projects"
$sampleProjectSrc = Join-Path $repoRoot "projects\sample"
$sampleProjectDst = Join-Path $projectsDir "sample"

Push-Location $repoRoot
try {
    cargo build --release -p belt_tools

    New-Item -ItemType Directory -Force -Path $toolsDir | Out-Null
    New-Item -ItemType Directory -Force -Path $projectsDir | Out-Null

    Copy-Item -Force `
        -Path (Join-Path $repoRoot "target\release\belt_tools.exe") `
        -Destination (Join-Path $toolsDir "belt_tools.exe")

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

    Write-Host "Packaged tools:"
    Write-Host "  $(Join-Path $toolsDir "belt_tools.exe")"
    Write-Host "Packaged sample project:"
    Write-Host "  $sampleProjectDst"
}
finally {
    Pop-Location
}
