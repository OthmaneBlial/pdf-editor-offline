param(
    [Parameter(Mandatory = $true)]
    [string]$AssetDirectory
)

$ErrorActionPreference = "Stop"
$installer = Get-ChildItem -Path $AssetDirectory -Filter "*.exe" | Select-Object -First 1
if (-not $installer) {
    throw "No NSIS installer found in $AssetDirectory"
}

$install = Start-Process -FilePath $installer.FullName -ArgumentList "/S" -Wait -PassThru
if ($install.ExitCode -ne 0) {
    throw "NSIS install failed with exit code $($install.ExitCode)"
}

$app = Get-ChildItem -Path $env:LOCALAPPDATA -Filter "pdf-editor-offline-desktop.exe" -File -Recurse |
    Select-Object -First 1
if (-not $app) {
    throw "Installed desktop executable was not found"
}

$process = Start-Process -FilePath $app.FullName -PassThru
$sidecarReady = $false
for ($attempt = 0; $attempt -lt 45; $attempt++) {
    if ($process.HasExited) {
        throw "Installed Windows application exited before startup"
    }
    if (Get-Process -Name "pdf-editor-offline-api" -ErrorAction SilentlyContinue) {
        $sidecarReady = $true
        break
    }
    Start-Sleep -Seconds 1
}
if (-not $sidecarReady) {
    throw "Installed Windows application did not start its local API sidecar"
}

$null = $process.CloseMainWindow()
if (-not $process.WaitForExit(10000)) {
    Stop-Process -Id $process.Id -Force
}
Start-Sleep -Seconds 2
if (Get-Process -Name "pdf-editor-offline-api" -ErrorAction SilentlyContinue) {
    Stop-Process -Name "pdf-editor-offline-api" -Force
    throw "Windows application left its exact sidecar running"
}

$uninstaller = Get-ChildItem -Path $app.Directory.Parent.FullName -Filter "uninstall.exe" -File -Recurse |
    Select-Object -First 1
if (-not $uninstaller) {
    throw "NSIS uninstaller was not found"
}
$uninstall = Start-Process -FilePath $uninstaller.FullName -ArgumentList "/S" -Wait -PassThru
if ($uninstall.ExitCode -ne 0) {
    throw "NSIS uninstall failed with exit code $($uninstall.ExitCode)"
}
if (Test-Path $app.FullName) {
    throw "Application executable remained after uninstall"
}

Write-Output "PASS: installed, launched, stopped, and uninstalled Windows application"
