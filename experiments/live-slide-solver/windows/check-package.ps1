[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Archive,
    [Parameter(Mandatory)] [string] $CellCheck,
    [Parameter(Mandatory)] [string] $OpticalPair
)
$ErrorActionPreference = 'Stop'
$zip = (Resolve-Path -LiteralPath $Archive).Path
$check = (Resolve-Path -LiteralPath $CellCheck).Path
$optics = (Resolve-Path -LiteralPath $OpticalPair).Path
$stage = [IO.Directory]::CreateDirectory((Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString()))).FullName
$oldPath = $env:Path
try {
    Expand-Archive -LiteralPath $zip -DestinationPath $stage
    foreach ($asset in 'the-sacred-swirls.exe','tss-performer.exe','tss-slide-solver.exe','cygwin1.dll','WINDOWS-QUICKSTART.md','PERFORMANCE-SURFACES.md','VISUAL-MEDIA.md','BASILISK-COPYING','CYGWIN-COPYING','CYGWIN-LICENSE') {
        $file = Get-Item -LiteralPath (Join-Path $stage $asset)
        if ($file.Length -eq 0) { throw "Empty archive member: $asset" }
        Write-Output "$asset $($file.Length) bytes $((Get-FileHash -LiteralPath $file.FullName).Hash)"
    }
    $worker = Join-Path $stage 'tss-slide-solver.exe'
    $env:Path = "$env:SystemRoot\system32;$env:SystemRoot"
    & $check $worker 6
    if ($LASTEXITCODE -ne 0) { throw "Windows cell protocol check exited $LASTEXITCODE." }
    & $optics $worker 5 176 48
    if ($LASTEXITCODE -ne 0) { throw "Windows optical pair check exited $LASTEXITCODE." }
    Write-Output "ZIP SHA-256 $((Get-FileHash -LiteralPath $zip).Hash)"
} finally {
    $env:Path = $oldPath
    Remove-Item -LiteralPath $stage -Recurse -Force
}
