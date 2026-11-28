# Run with the source tree and the two x64 release executables present.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidatePattern('^v[0-9][A-Za-z0-9.-]*$')] [string] $Tag,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [string] $BinaryDirectory,
    [ValidatePattern('^[a-fA-F0-9]{7,40}$')] [string] $DocumentationCommit,
    # Root of the curated example tree. Paths inside it are preserved under examples/.
    [string] $ExampleDirectory,
    # Output of build-windows-solver.ps1 paired with a live-solver feature build.
    [string] $LiveSolverDirectory,
    # Set to an empty string for a release without a suggested Legends collection.
    [AllowEmptyString()][string] $LegendPackDirectory = (Join-Path $PSScriptRoot '../legend-packs/research')
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (!$BinaryDirectory) { $BinaryDirectory = Join-Path $repo 'src-tauri/target/release' }
$bin = (Resolve-Path -LiteralPath $BinaryDirectory).Path
if (!$LiveSolverDirectory -and (Test-Path -LiteralPath (Join-Path $bin 'tss-slide-solver.exe'))) {
    $LiveSolverDirectory = $bin
}
$out = [IO.Directory]::CreateDirectory($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)).FullName
$name = "The-Sacred-Swirls-$Tag-Windows-x64.zip"
$target = Join-Path $out $name
if (Test-Path -LiteralPath $target) { throw "Archive already exists: $target" }
$scratch = [IO.Directory]::CreateDirectory((Join-Path $out ([Guid]::NewGuid().ToString()))).FullName
try {
    foreach ($exe in 'the-sacred-swirls.exe','tss-performer.exe') {
        Copy-Item -LiteralPath (Join-Path $bin $exe) -Destination $scratch
    }
    if ($LiveSolverDirectory) {
        $solver = (Resolve-Path -LiteralPath $LiveSolverDirectory).Path
        foreach ($member in 'tss-slide-solver.exe','cygwin1.dll','BASILISK-COPYING','CYGWIN-COPYING','CYGWIN-LICENSE') {
            Copy-Item -LiteralPath (Join-Path $solver $member) -Destination $scratch
        }
    }
    $quickstart = Get-Content (Join-Path $repo 'docs/windows-preview.md') -Raw -Encoding UTF8
    $ref = if ($DocumentationCommit) { $DocumentationCommit } else { $Tag }
    $base = "https://github.com/GNPNG/tss/blob/$ref/"
    $quickstart = $quickstart.Replace('](../', "]($base").Replace('](performance-surfaces.md)', '](PERFORMANCE-SURFACES.md)').Replace('](visual-media.md)', '](VISUAL-MEDIA.md)')
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $scratch 'WINDOWS-QUICKSTART.md'), $quickstart, $utf8)
    Copy-Item -LiteralPath (Join-Path $repo 'docs/performance-surfaces.md') -Destination (Join-Path $scratch 'PERFORMANCE-SURFACES.md')
    $media = Get-Content (Join-Path $repo 'docs/visual-media.md') -Raw -Encoding UTF8
    [IO.File]::WriteAllText((Join-Path $scratch 'VISUAL-MEDIA.md'), $media.Replace('](windows-preview.md)', '](WINDOWS-QUICKSTART.md)'), $utf8)
    if ($ExampleDirectory) {
        $example = Get-Item -LiteralPath $ExampleDirectory
        if (!$example.PSIsContainer) { throw "ExampleDirectory must be a directory: $ExampleDirectory" }
        Copy-Item -LiteralPath $example.FullName -Destination (Join-Path $scratch 'examples') -Recurse
    }
    if ($LegendPackDirectory -and (Test-Path -LiteralPath $LegendPackDirectory -PathType Container)) {
        $packs = Get-ChildItem -LiteralPath $LegendPackDirectory -File -Filter '*.tsslegends.json' |
            Where-Object { -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) }
        if ($packs) {
            $catalog = [IO.Directory]::CreateDirectory((Join-Path $scratch 'legend-packs')).FullName
            foreach ($pack in $packs) { Copy-Item -LiteralPath $pack.FullName -Destination $catalog }
        }
    }
    $files = @(Get-ChildItem -LiteralPath $scratch | Sort-Object Name | ForEach-Object FullName)
    $archive = Join-Path $scratch $name
    Compress-Archive -LiteralPath $files -DestinationPath $archive -CompressionLevel Optimal
    [IO.File]::Move($archive, $target)
    Get-FileHash -Algorithm SHA256 -LiteralPath $target
} finally {
    Remove-Item -LiteralPath $scratch -Recurse -Force
}
