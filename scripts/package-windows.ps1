# Run with the source tree and the two x64 release executables present.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidatePattern('^v[0-9][A-Za-z0-9.-]*$')] [string] $Tag,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [string] $BinaryDirectory = (Join-Path $PSScriptRoot '../src-tauri/target/release'),
    [ValidatePattern('^[a-fA-F0-9]{7,40}$')] [string] $DocumentationCommit
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$bin = (Resolve-Path $BinaryDirectory).Path
$out = [IO.Directory]::CreateDirectory($OutputDirectory).FullName
$name = "The-Sacred-Swirls-$Tag-Windows-x64.zip"
$target = Join-Path $out $name
if (Test-Path -LiteralPath $target) { throw "Archive already exists: $target" }
$scratch = [IO.Directory]::CreateDirectory((Join-Path $out ([Guid]::NewGuid().ToString()))).FullName
try {
    foreach ($exe in 'the-sacred-swirls.exe','tss-performer.exe') {
        Copy-Item -LiteralPath (Join-Path $bin $exe) -Destination $scratch
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
    $files = @(Get-ChildItem -LiteralPath $scratch -File | Sort-Object Name | ForEach-Object FullName)
    $archive = Join-Path $scratch $name
    Compress-Archive -LiteralPath $files -DestinationPath $archive -CompressionLevel Optimal
    [IO.File]::Move($archive, $target)
    Get-FileHash -Algorithm SHA256 -LiteralPath $target
} finally {
    Remove-Item -LiteralPath $scratch -Recurse -Force
}
