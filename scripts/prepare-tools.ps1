param([string]$ArchivePath)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$scratch = Join-Path $projectRoot '.local/tools'
New-Item -ItemType Directory -Path $scratch -Force | Out-Null
if (!$ArchivePath) {
    $ArchivePath = Join-Path $scratch 'ffmpeg-release-essentials.zip'
    if (!(Test-Path -LiteralPath $ArchivePath)) {
        Invoke-WebRequest -Uri 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip' -OutFile $ArchivePath
    }
}
$expected = '60F467265B1E312373DBCD92200C2618A74850F98D3D078E94296BB3FA2047BA'
if ((Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash -ne $expected) {
    throw 'FFmpeg archive differs from tested release. Do not bypass: update the pinned checksum and license information after validating the new build.'
}
$expanded = Join-Path $scratch 'expanded'
Expand-Archive -LiteralPath $ArchivePath -DestinationPath $expanded -Force
$ffmpeg = Get-ChildItem -LiteralPath $expanded -Filter ffmpeg.exe -Recurse | Select-Object -First 1
if (!$ffmpeg) { throw 'ffmpeg.exe missing in archive' }
$bin = Join-Path $projectRoot 'apps/desktop/src-tauri/binaries'
New-Item -ItemType Directory -Path $bin -Force | Out-Null
foreach ($name in @('ffmpeg','ffprobe')) {
    Copy-Item -LiteralPath (Join-Path $ffmpeg.DirectoryName "$name.exe") -Destination (Join-Path $bin "$name-x86_64-pc-windows-msvc.exe") -Force
}
$upstreamRoot = Split-Path -Parent $ffmpeg.DirectoryName
foreach ($name in @('LICENSE','README.txt')) {
    Copy-Item -LiteralPath (Join-Path $upstreamRoot $name) -Destination (Join-Path $projectRoot "docs/licenses/$name") -Force
}
Write-Host 'FFmpeg/ffprobe ready; archive checksum verified.'
