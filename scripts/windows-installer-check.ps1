param([string]$Installer = "dist\Wega-Windows-x64-Setup.exe")
$ErrorActionPreference = 'Stop'
$registration = 'HKCU:\Software\Clients\StartMenuInternet\Wega'
if (Test-Path $registration) { throw 'Refusing to replace an existing Wega registration in this test' }
$destination = Join-Path $env:TEMP ('Wega-install-check-' + [guid]::NewGuid().ToString('N'))
try {
    $process = Start-Process -FilePath (Resolve-Path $Installer) -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DIR=`"$destination`"") -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
    foreach ($file in 'wega.exe', 'libEGL.dll', 'libGLESv2.dll', 'LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES.html', 'unins000.exe') {
        if (!(Test-Path (Join-Path $destination $file))) { throw "Installed file missing: $file" }
    }
    $capabilities = Get-ItemProperty "$registration\Capabilities\URLAssociations"
    if ($capabilities.http -ne 'WegaURL' -or $capabilities.https -ne 'WegaURL') { throw 'URL capabilities are missing' }
    $command = (Get-Item 'HKCU:\Software\Classes\WegaURL\shell\open\command').GetValue('')
    if ($command -ne "`"$destination\wega.exe`" -- `"%1`"") { throw "Unexpected URL launch command: $command" }
    $startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Wega\Wega.lnk'
    if (!(Test-Path $startMenu)) { throw 'Start menu shortcut is missing' }
} finally {
    $uninstaller = Join-Path $destination 'unins000.exe'
    if (Test-Path $uninstaller) {
        $process = Start-Process $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Uninstall failed: $($process.ExitCode)" }
    }
}
if (Test-Path $registration) { throw 'Uninstall left browser registration behind' }
if (Test-Path 'HKCU:\Software\Classes\WegaURL') { throw 'Uninstall left the URL handler behind' }
Write-Output 'PASS: per-user installation, packaged files, browser registration, shortcut and uninstall'
