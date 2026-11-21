# qcc translates Basilisk C. Cygwin supplies the worker's POSIX facilities in a
# Windows x64 executable. WSL is required on the build host only.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $BasiliskSourceWsl,
    [Parameter(Mandatory)] [string] $CygwinRoot,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [string] $WslDistribution
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cygwin = (Resolve-Path -LiteralPath $CygwinRoot).Path
$bin = Join-Path $cygwin 'bin'
$gcc = Join-Path $bin 'gcc.exe'
$runtimeDll = Join-Path $bin 'cygwin1.dll'
foreach ($file in $gcc,$runtimeDll) {
    if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing Cygwin build dependency: $file" }
}
$out = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $out) { throw "Output already exists: $out" }
$parent = [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($out)).FullName
$stage = [IO.Directory]::CreateDirectory((Join-Path $parent ([Guid]::NewGuid().ToString()))).FullName
$payload = [IO.Directory]::CreateDirectory((Join-Path $stage 'dist')).FullName
$dist = @()
if ($WslDistribution) { $dist = @('--distribution',$WslDistribution) }
try {
    Copy-Item -LiteralPath (Join-Path $repo 'experiments/slide-look-study/solver.c') -Destination $stage
    $linuxStage = (& wsl.exe @dist --cd $stage --exec pwd).Trim()
    if ($LASTEXITCODE -ne 0 -or !$linuxStage.StartsWith('/')) { throw 'Cannot map scratch directory into WSL.' }
    & wsl.exe @dist --cd $linuxStage --exec env 'LC_ALL=C' "BASILISK=$BasiliskSourceWsl" "$BasiliskSourceWsl/qcc" -cpu -source -O2 -Wall -disable-dimensions solver.c
    if ($LASTEXITCODE -ne 0) { throw "Basilisk translation exited $LASTEXITCODE." }
    & wsl.exe @dist --cd $linuxStage --exec cp -- "$BasiliskSourceWsl/COPYING" dist/BASILISK-COPYING
    if ($LASTEXITCODE -ne 0) { throw "Cannot stage Basilisk's licence." }
    $cygwinDocs = Join-Path $cygwin 'usr/share/doc/cygwin'
    Copy-Item -LiteralPath (Join-Path $cygwinDocs 'COPYING') -Destination (Join-Path $payload 'CYGWIN-COPYING')
    Copy-Item -LiteralPath (Join-Path $cygwinDocs 'CYGWIN_LICENSE') -Destination (Join-Path $payload 'CYGWIN-LICENSE')
    $oldPath = $env:Path
    Push-Location $stage
    try {
        $env:Path = "$bin;$oldPath"
        if ((& $gcc -dumpmachine) -ne 'x86_64-pc-cygwin') { throw 'The worker requires the Windows x64 Cygwin compiler.' }
        & $gcc '-std=gnu99' '-O2' '-static-libgcc' '_solver.c' '-o' 'dist/tss-slide-solver.exe' '-lm'
        if ($LASTEXITCODE -ne 0) { throw "Windows worker compilation exited $LASTEXITCODE." }
    } finally {
        $env:Path = $oldPath
        Pop-Location
    }
    Copy-Item -LiteralPath $runtimeDll -Destination $payload
    [IO.Directory]::Move($payload, $out)
} finally {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
