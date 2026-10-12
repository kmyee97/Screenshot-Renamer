[CmdletBinding()]
param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$previousDirectory = Get-Location
try {
    Set-Location -LiteralPath $repositoryRoot
    if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64' -or -not $IsWindows) {
        throw 'Build this release on 64-bit Windows with PowerShell 7.'
    }
    $dirty = git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Use a clean committed checkout to build a release.' }
    $version = (Get-Content -LiteralPath package.json -Raw | ConvertFrom-Json).version
    node scripts/bump-version.mjs $version --check
    if ($LASTEXITCODE -ne 0) { throw 'Application versions are inconsistent.' }
    if (-not $SkipTests) {
        npm test
        if ($LASTEXITCODE -ne 0) { throw 'Frontend tests failed.' }
        cargo test --manifest-path src-tauri/Cargo.toml --locked
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    }
    npm run tauri build -- --bundles nsis --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Windows installer build failed.' }
    $bundleDirectory = Join-Path $repositoryRoot 'src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis'
    $artifact = Get-ChildItem -LiteralPath $bundleDirectory -File -Filter "*_${version}_x64-setup.exe" | Select-Object -First 1
    if (-not $artifact) { throw 'The expected NSIS installer was not produced.' }
    $outputDirectory = Join-Path $repositoryRoot 'release-artifacts'
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    $outputPath = Join-Path $outputDirectory $artifact.Name
    Copy-Item -LiteralPath $artifact.FullName -Destination $outputPath
    $manifest = [ordered]@{
        version = $version
        artifact = $artifact.Name
        sha256 = (Get-FileHash -LiteralPath $outputPath -Algorithm SHA256).Hash
        commit = (git rev-parse HEAD)
        architecture = 'x86_64-pc-windows-msvc'
        builtAtUtc = (Get-Date).ToUniversalTime().ToString('o')
        installMode = 'currentUser'
        signed = $false
        acceptance = 'Pending clean-machine validation'
    }
    $manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outputDirectory "$($artifact.BaseName).release.json") -Encoding utf8
    Write-Output "Installer: $outputPath"
    Write-Output "SHA256: $($manifest.sha256)"
} finally { Set-Location -LiteralPath $previousDirectory }
