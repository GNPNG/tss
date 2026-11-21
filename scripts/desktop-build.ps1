# Build worker and performer in the same profile directory as the desktop editor.
param([Parameter(Mandatory)] [ValidateSet('dev','build')] [string] $Mode)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$profileName = if ($Mode -eq 'dev') { 'debug' } else { 'release' }
if (!$env:CIMG_INCLUDE -or !(Test-Path -LiteralPath (Join-Path $env:CIMG_INCLUDE 'CImg.h'))) {
    throw 'Set CIMG_INCLUDE to the directory containing CImg.h.'
}
Push-Location $repo
try {
    $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'src-tauri/target' }
    $env:CARGO_TARGET_DIR = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($target)
    $bin = [IO.Directory]::CreateDirectory((Join-Path $env:CARGO_TARGET_DIR $profileName)).FullName
    $solver = $env:TSS_LIVE_SOLVER_DIR
    $scratch = $null
    try {
        if (!$solver) {
            if (!$env:BASILISK_WSL -or !$env:CYGWIN_ROOT) {
                throw 'Set BASILISK_WSL and CYGWIN_ROOT to build the solver, or set TSS_LIVE_SOLVER_DIR to its compiled Windows payload.'
            }
            $scratch = [IO.Directory]::CreateDirectory((Join-Path $bin ([guid]::NewGuid().ToString()))).FullName
            $solver = Join-Path $scratch 'solver'
            & "$PSScriptRoot/build-windows-solver.ps1" -BasiliskSourceWsl $env:BASILISK_WSL -CygwinRoot $env:CYGWIN_ROOT -WslDistribution $env:TSS_WSL_DISTRIBUTION -OutputDirectory $solver
        }
        $solver = (Resolve-Path -LiteralPath $solver).Path
        foreach ($file in 'tss-slide-solver.exe','cygwin1.dll','BASILISK-COPYING','CYGWIN-COPYING','CYGWIN-LICENSE') {
            $source = Get-Item -LiteralPath (Join-Path $solver $file) -ErrorAction Stop
            if ($bin -ne $solver) { Copy-Item -LiteralPath $source.FullName -Destination $bin -Force }
        }
    } finally { if ($scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force } }
    & go -C performer build -trimpath -o (Join-Path $bin 'tss-performer.exe') .
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if ($Mode -eq 'dev') {
        & cargo run --locked --manifest-path src-tauri/Cargo.toml --features live-solver @args
    } else {
        & cargo build --locked --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol,live-solver @args
    }
    exit $LASTEXITCODE
} finally { Pop-Location }
