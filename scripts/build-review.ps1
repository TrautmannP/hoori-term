param([string]$BuildRoot = 'C:\Repos\hoori-term-builds')
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
if (git status --porcelain --untracked-files=normal) { throw 'Review builds require clean committed source.' }
$commit = (git rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $commit -notmatch '^[0-9a-f]{40}$') { throw 'Cannot identify source commit.' }
$root = [IO.Path]::GetFullPath($BuildRoot)
$destination = Join-Path $root $commit
if (Test-Path -LiteralPath $destination) { throw "Refusing to overwrite $destination" }
cargo build --release --locked --no-default-features
if ($LASTEXITCODE -ne 0) { throw 'Windows release build failed.' }
if ((git rev-parse HEAD).Trim() -ne $commit -or (git status --porcelain --untracked-files=normal)) {
    throw 'Source changed while building; refusing to publish.'
}
$targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
$binary = Join-Path $targetRoot 'release\hoori-term.exe'
$metadata = (& $binary --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $metadata -notlike "*commit: $commit*" -or $metadata -notlike '*dirty: false*' -or $metadata -notlike '*target: x86_64-pc-windows-msvc*') {
    throw 'Embedded build identity does not match clean Windows x64 source.'
}
New-Item -ItemType Directory -Path $root -Force | Out-Null
$staging = Join-Path $root ('.build-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $staging | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $staging 'hoori-term.exe')
Copy-Item -LiteralPath 'README.md' -Destination (Join-Path $staging 'README.md')
$os = Get-CimInstance Win32_OperatingSystem
Set-Content -LiteralPath (Join-Path $staging 'BUILD-METADATA.txt') -Value ($metadata + "`nbuild host: $($os.Caption) $($os.Version) $($os.BuildNumber)") -Encoding utf8
Set-Content -LiteralPath (Join-Path $staging 'launch.ps1') -Value '& (Join-Path $PSScriptRoot ''hoori-term.exe'') @args' -Encoding utf8
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $staging 'hoori-term.lnk'))
$shortcut.TargetPath = Join-Path $destination 'hoori-term.exe'
$shortcut.WorkingDirectory = $destination
$shortcut.Description = "hoori-term [$($commit.Substring(0,7))] - Windows x64 review"
$shortcut.Save()
# Both directory paths must remain direct children of the requested build root.
foreach ($candidate in @($staging, $destination)) {
    if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($candidate)) -ne $root.TrimEnd('\')) {
        throw 'Review artifact escaped its build root.'
    }
}
Move-Item -LiteralPath $staging -Destination $destination
Write-Output "Review build: $destination"
Write-Output "Launch: $destination\hoori-term.lnk"
Write-Output "Command: & '$destination\launch.ps1'"
