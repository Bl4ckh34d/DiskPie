$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
$previousFlags = $env:CARGO_ENCODED_RUSTFLAGS
try {
    # Strip machine-specific build paths from the shipped executable.
    $flags = @('-C', 'target-feature=+crt-static', '--remap-path-prefix', "$repoRoot=.", '--remap-path-prefix', "$env:USERPROFILE=/builder")
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]31
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) { $cargoExe = $cargo.Source } else { $cargoExe = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    & $cargoExe build --locked --release
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $output = Join-Path $repoRoot 'dist\DiskPie'
    New-Item -ItemType Directory -Force $output | Out-Null
    Copy-Item -LiteralPath 'target\release\DiskPie.exe' -Destination (Join-Path $output 'DiskPie.exe') -Force
    Copy-Item -LiteralPath 'README.md' -Destination (Join-Path $output 'README.md') -Force
    python scripts/third_party_notices.py --target x86_64-pc-windows-msvc --output (Join-Path $output 'THIRD_PARTY_NOTICES.txt') --cargo $cargoExe
    if ($LASTEXITCODE -ne 0) { throw 'Dependency license packaging failed' }
    # Explicit file list: never package arbitrary directory contents.
    Compress-Archive -LiteralPath @((Join-Path $output 'DiskPie.exe'), (Join-Path $output 'README.md'), (Join-Path $output 'THIRD_PARTY_NOTICES.txt')) -DestinationPath 'dist\DiskPie-windows-x64.zip' -Force
    Get-FileHash -Algorithm SHA256 'dist\DiskPie-windows-x64.zip'
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $previousFlags
    Pop-Location
}
