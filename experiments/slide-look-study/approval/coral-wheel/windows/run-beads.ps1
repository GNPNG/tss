param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $Workspace,
    [Parameter(Mandatory)][string] $RunDirectory,
    [Parameter(Mandatory)][string] $ExpectedNvidiaUuid,
    [Parameter(Mandatory)][ValidateSet(640,960)][int] $PreviewWidth,
    [switch] $WheelStack
)
$ErrorActionPreference = 'Stop'
$code = 1
try {
    Set-Location -LiteralPath $RunDirectory
    if ((Get-Process -Id $PID).SessionId -eq 0) { throw 'Interactive Windows desktop required.' }
    if (Get-Process -Name the-sacred-swirls,tss-gpu-job -ErrorAction SilentlyContinue) { throw 'An editor or GPU guard is already running.' }
    if (Test-Path (Join-Path $Workspace 'gpu-job-control\reservation')) { throw 'Python GPU reservation is present.' }

    $guardDirectory = Join-Path $Workspace 'gpu-job-control-rust'
    $guard = Join-Path $guardDirectory 'target\debug\tss-gpu-job.exe'
    $parameters = @(
        'run', '--reservation', (Join-Path $guardDirectory 'reservation'),
        '--timeout', '120', '--telemetry-csv', (Join-Path $RunDirectory 'gpu.csv'),
        '--interval', '1', '--require-idle', '--expected-nvidia-uuid', $ExpectedNvidiaUuid,
        '--output-log', (Join-Path $RunDirectory 'editor.log'), '--',
        $Executable, '--benchmark-live', (Join-Path $RunDirectory 'benchmark.json')
    )
    if ($WheelStack) { $parameters += '--wheel-stack-trial' } else { $parameters += '--coral-beads-trial' }
    if ($PreviewWidth -eq 960) { $parameters += '--preview-960' }
    $ErrorActionPreference = 'Continue'
    & $guard @parameters 1> (Join-Path $RunDirectory 'guard.log') 2> (Join-Path $RunDirectory 'guard-errors.log')
    $code = $LASTEXITCODE
} catch {
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $RunDirectory 'launch-error.txt')
}
[IO.File]::WriteAllText((Join-Path $RunDirectory 'exit.txt'), [string]$code)
exit $code
