param(
    [string]$OutRoot = "dist"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$release = Join-Path $root "target\release"
$stamp = Get-Date -Format "yyyyMMdd"
$stage = Join-Path $root "$OutRoot\imgcomp-$stamp"

if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

$items = @(
    @{ src = "imgcomp.exe";     name = "imgcomp.exe" },
    @{ src = "imgcomp-gui.exe"; name = "imgcomp-gui.exe" }
)
Copy-Item (Join-Path $root "README.md") (Join-Path $stage "README.md") -Force

foreach ($i in $items) {
    $p = Join-Path $release $i.src
    if (Test-Path $p) {
        Copy-Item $p (Join-Path $stage $i.name) -Force
    }
}

$zip = Join-Path $root "$OutRoot\imgcomp-$stamp.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -Force

$cli = Get-Item (Join-Path $stage "imgcomp.exe") -ErrorAction SilentlyContinue
$gui = Get-Item (Join-Path $stage "imgcomp-gui.exe") -ErrorAction SilentlyContinue
$zipSize = (Get-Item $zip).Length

Write-Host "Packaged -> $zip"
if ($cli) { Write-Host ("  imgcomp.exe      {0:N2} MB" -f ($cli.Length / 1MB)) }
if ($gui) { Write-Host ("  imgcomp-gui.exe  {0:N2} MB" -f ($gui.Length / 1MB)) }
Write-Host ("  zip              {0:N2} MB" -f ($zipSize / 1MB))
