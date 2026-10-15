param([switch]$Package)
$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
Push-Location $projectRoot
try {
    New-Item -ItemType Directory -Force -Path 'dist', 'verification' | Out-Null
    & cargo build --manifest-path upstream/Cargo.toml --release -p stellaris-parser --locked
    if ($LASTEXITCODE) { throw 'Native parser build failed' }
    $metadata = & cargo metadata --manifest-path upstream/Cargo.toml --no-deps --format-version 1
    if ($LASTEXITCODE) { throw 'Cargo metadata failed' }
    $targetDirectory = ($metadata | ConvertFrom-Json).target_directory
    Copy-Item -LiteralPath (Join-Path $targetDirectory 'release/stellaris-parser.exe') -Destination 'dist/stellaris-parser.exe'
    & python -X utf8 -m unittest discover -s tests -v
    if ($LASTEXITCODE) { throw 'Regression tests failed' }
    if ($Package) {
        & python -X utf8 -m PyInstaller --noconfirm --onedir --name tiger-stellar --distpath dist --workpath build --specpath build --paths src --add-data 'rules;rules' --add-binary 'dist/stellaris-parser.exe;.' src/stellar.py
        if ($LASTEXITCODE) { throw 'Application packaging failed' }
        Copy-Item -LiteralPath LICENSE -Destination dist/tiger-stellar/LICENSE
        Copy-Item -LiteralPath README.md -Destination dist/tiger-stellar/README.md
    }
} finally {
    Pop-Location
}
