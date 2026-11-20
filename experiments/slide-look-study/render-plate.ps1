param(
    [Parameter(Mandatory)] [string] $InputFile,
    [Parameter(Mandatory)] [string] $OutputFile,
    [Parameter(Mandatory)] [string] $FFmpeg,
    [Parameter(Mandatory)] [string] $JobGuard,
    [Parameter(Mandatory)] [string] $Reservation,
    [Parameter(Mandatory)] [string] $Adapter,
    [Parameter(Mandatory)] [string] $NvidiaUuid,
    [int] $Width = 1280,
    [int] $TimeoutSeconds = 180,
    [string] $Wheel = 'violet-blue',
    [string] $PresetFile
)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$tag = [guid]::NewGuid().ToString()

# Convert photographed display colours to linear RGB before deriving densities.
function ConvertTo-LinearRgb([string] $Hex) {
    if ($Hex -notmatch '^#[0-9a-fA-F]{6}$') { throw "Invalid sRGB swatch '$Hex'" }
    foreach ($offset in 1,3,5) {
        $v = [Convert]::ToInt32($Hex.Substring($offset, 2), 16) / 255.0
        if ($v -le 0.04045) { $v / 12.92 } else { [Math]::Pow(($v + 0.055) / 1.055, 2.4) }
    }
}
function Get-Density([double[]] $Sample, [double[]] $Carrier) {
    foreach ($i in 0..2) {
        $ratio = $Sample[$i] / [Math]::Max(0.00001, $Carrier[$i])
        -[Math]::Log([Math]::Min(1.0, [Math]::Max(0.00001, $ratio)))
    }
}
function Format-Vector([double[]] $Numbers) {
    $parts = foreach ($v in $Numbers) { $v.ToString('0.00000000', [CultureInfo]::InvariantCulture) }
    return 'vec3(' + ($parts -join ',') + ')'
}
$hook = 'optics.hook'
if ($Wheel -ne 'violet-blue') {
    if (!$PresetFile) {
        $PresetFile = Join-Path $PSScriptRoot '../../WebAssets/assets/mathmos-wheels.json'
        if (!(Test-Path -LiteralPath $PresetFile)) {
            $PresetFile = Join-Path $PSScriptRoot 'mathmos-wheels.json'
        }
    }
    $registry = Get-Content -Raw -LiteralPath $PresetFile | ConvertFrom-Json
    if ($registry.version -ne 1) { throw 'Unsupported wheel registry version' }
    $matchesById = @($registry.wheels | Where-Object { $_.id -ceq $Wheel })
    if ($matchesById.Count -ne 1) { throw "Expected one wheel with id '$Wheel' in $PresetFile" }
    $preset = $matchesById[0]
    $color = $preset.swatches
    $carrier = @(ConvertTo-LinearRgb $color.carrier)
    $rear = @(Get-Density (ConvertTo-LinearRgb $color.rear) $carrier)
    $front = @(Get-Density (ConvertTo-LinearRgb $color.front) $carrier)
    $overlap = @(Get-Density (ConvertTo-LinearRgb $color.overlap) $carrier)
    $rim = if ($preset.rearRim) { Get-Density (ConvertTo-LinearRgb $preset.rearRim) $carrier } else { $rear }
    $paired = foreach ($i in 0..2) {
        # The effective joint attenuation is at least that of either slide.
        [Math]::Max($overlap[$i], [Math]::Max($rear[$i], $front[$i])) - $rear[$i] - $front[$i]
    }
    $rimDelta = foreach ($i in 0..2) { $rim[$i] - $rear[$i] }
    if ($null -eq $preset.depthVariation -or $preset.depthVariation -lt 0 -or $preset.depthVariation -gt 1) {
        throw "Expected a depth variation in [0,1] for '$Wheel'"
    }
    $variation = ([double]$preset.depthVariation).ToString('0.00000000', [CultureInfo]::InvariantCulture)
    $material = @(
        '#define TSS_WHEEL_PROFILE 1'
        "const vec3 pinkAbsorption = $(Format-Vector $rear);"
        "const vec3 cyanAbsorption = $(Format-Vector $front);"
        "const vec3 pairedDensityCorrection = $(Format-Vector $paired);"
        "const vec3 rearRimDelta = $(Format-Vector $rimDelta);"
        "const vec3 wheelCarrier = $(Format-Vector $carrier);"
        "const float depthVariation = $variation;"
    ) -join [Environment]::NewLine
    $template = Get-Content -LiteralPath $hook -Raw
    $marker = '// TSS_WHEEL_MATERIAL'
    if (!$template.Contains($marker)) { throw "Missing material insertion point in $hook" }
    $hook = "wheel-$tag.hook"
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot $hook), $template.Replace($marker,$material))
}
$height = [int] ($Width * 9 / 16)
$still = $OutputFile.EndsWith('.png')
# Both SDFs are data channels. Keep full-range RGB until the projector hook.
$graph = "scale=${Width}:${height}:flags=bicubic,format=rgba64le,setparams=range=full:colorspace=gbr:color_primaries=bt709:color_trc=iec61966-2-1,hwupload,libplacebo=custom_shader_path=${hook}:format=rgba64le:color_trc=iec61966-2-1:color_primaries=bt709:dithering=none,hwdownload,format=rgba64le"
$encode = if ($still) {
    @('-pix_fmt','rgb48be','-c:v','png','-threads','2')
} else {
    # Convert the shader's display sRGB into limited-range BT.709 for preview.
    $graph += ',format=gbrp16le,zscale=pin=709:tin=iec61966-2-1:min=gbr:rin=full:p=709:t=709:m=709:r=limited,format=yuv420p'
    @('-c:v','libx264','-preset','veryfast','-crf','16','-threads','2','-movflags','+faststart','-colorspace','bt709','-color_primaries','bt709','-color_trc','bt709','-color_range','tv')
}
$guardArgs = @('run','--reservation',$Reservation,'--timeout',"$TimeoutSeconds",'--telemetry-csv',"telemetry-$tag.csv",'--expected-nvidia-uuid',$NvidiaUuid,'--require-idle','--output-log',"render-$tag.log",'--')
$renderArgs = @('-nostdin','-hide_banner','-loglevel','verbose','-n','-filter_threads','2','-init_hw_device',"vulkan=plate:$Adapter",'-filter_hw_device','plate','-i',$InputFile,'-an','-vf',$graph)
try {
    & $JobGuard @guardArgs $FFmpeg @renderArgs @encode $OutputFile
    $code = $LASTEXITCODE
} finally {
    if ($hook -ne 'optics.hook') { Remove-Item -LiteralPath $hook }
}
Write-Output "Render log $PSScriptRoot/render-$tag.log"
if ($code -ne 0) { Get-Content "render-$tag.log" -Tail 22 }
exit $code
