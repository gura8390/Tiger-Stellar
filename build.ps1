[CmdletBinding()]
param(
    [string]$CargoHome,
    [string]$TargetDir
)

$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
$buildRoot = Join-Path $projectRoot 'build'
$manifest = Join-Path $projectRoot 'upstream/Cargo.toml'

function Resolve-ProjectPath([string]$PathValue, [string]$DefaultValue) {
    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return [IO.Path]::GetFullPath($DefaultValue)
    }
    if ([IO.Path]::IsPathRooted($PathValue)) {
        return [IO.Path]::GetFullPath($PathValue)
    }
    return [IO.Path]::GetFullPath((Join-Path $projectRoot $PathValue))
}

if ([string]::IsNullOrWhiteSpace($CargoHome)) { $CargoHome = $env:CARGO_HOME }
if ([string]::IsNullOrWhiteSpace($TargetDir)) { $TargetDir = $env:CARGO_TARGET_DIR }
$selectedCargoHome = Resolve-ProjectPath $CargoHome (Join-Path $buildRoot 'cargo-home')
$selectedTargetDir = Resolve-ProjectPath $TargetDir (Join-Path $buildRoot 'target')
$temporaryDirectory = Join-Path $buildRoot 'temp'
$binaryDirectory = Join-Path $buildRoot 'bin'
$cargo = (Get-Command cargo -CommandType Application -ErrorAction Stop).Source
$environmentNames = @('CARGO_HOME', 'CARGO_TARGET_DIR', 'TEMP', 'TMP', 'TMPDIR', 'RUSTUP_AUTO_INSTALL', 'TIGER_STELLAR_TEST_EXE')
$previousEnvironment = @{}
foreach ($name in $environmentNames) {
    $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
$locationPushed = $false

try {
    foreach ($path in @($buildRoot, $selectedCargoHome, $selectedTargetDir, $temporaryDirectory, $binaryDirectory)) {
        [void][IO.Directory]::CreateDirectory($path)
    }
    $env:CARGO_HOME = $selectedCargoHome
    $env:CARGO_TARGET_DIR = $selectedTargetDir
    $env:TEMP = $temporaryDirectory
    $env:TMP = $temporaryDirectory
    $env:TMPDIR = $temporaryDirectory
    $env:RUSTUP_AUTO_INSTALL = '0'
    Push-Location $projectRoot
    $locationPushed = $true

    # Cargo's artifact messages identify the executable even when an explicit
    # target triple or an external target directory is selected.
    $messages = @(& $cargo build --manifest-path $manifest --workspace --release --locked --message-format=json-render-diagnostics)
    if ($LASTEXITCODE -ne 0) { throw 'Rust workspace build failed' }
    $builtExecutable = $null
    foreach ($message in $messages) {
        try { $record = $message | ConvertFrom-Json } catch { continue }
        if ($record.reason -eq 'compiler-artifact' -and $record.target.name -eq 'tiger-stellar' -and
            $record.target.kind -contains 'bin' -and $record.executable) {
            $builtExecutable = [string]$record.executable
        }
    }
    if (-not $builtExecutable -or -not (Test-Path -LiteralPath $builtExecutable -PathType Leaf)) {
        throw 'Cargo did not return the Tiger Stellar executable'
    }

    # Integration tests must use this build, not an inherited older checker.
    $env:TIGER_STELLAR_TEST_EXE = $builtExecutable
    & $cargo test --manifest-path $manifest --workspace --release --locked -- --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Rust workspace tests failed' }

    $output = Join-Path $binaryDirectory 'tiger-stellar.exe'
    Copy-Item -LiteralPath $builtExecutable -Destination $output -Force
    $hash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(
        (Join-Path $binaryDirectory 'tiger-stellar.exe.sha256'),
        "$hash  tiger-stellar.exe`n",
        [Text.UTF8Encoding]::new($false)
    )
    Write-Output "Build and workspace tests passed: $output"
    Write-Output "SHA256: $hash"
} finally {
    if ($locationPushed) { Pop-Location }
    foreach ($name in $environmentNames) {
        [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
    }
}
