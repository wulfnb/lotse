# Starts wega.exe from a directory that also holds the ANGLE DLLs and fails if it
# exits during startup. CI runners have no GPU driver, so this catches GPU-context
# and missing-DLL failures that unit tests cannot.
param(
    [Parameter(Mandatory)] [string] $Directory,
    [int] $Seconds = 30
)
$ErrorActionPreference = "Stop"

foreach ($file in "wega.exe", "libEGL.dll", "libGLESv2.dll") {
    if (!(Test-Path (Join-Path $Directory $file))) {
        throw "$file is missing from $Directory"
    }
}
$profileDir = Join-Path $env:RUNNER_TEMP "wega-launch-profile"
$log = Join-Path $env:RUNNER_TEMP "wega-launch.err"
$process = Start-Process (Join-Path $Directory "wega.exe") -ArgumentList "--profile-dir", $profileDir, "about:blank" `
    -RedirectStandardError $log -PassThru
Start-Sleep -Seconds $Seconds
$exited = $process.HasExited
if (!$exited) { Stop-Process -Id $process.Id -Force }
Get-Content $log -ErrorAction SilentlyContinue
if ($exited) {
    throw "wega.exe exited during startup with code $($process.ExitCode)"
}
Write-Output "wega.exe was still running after $Seconds seconds"
