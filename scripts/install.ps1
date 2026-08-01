$ErrorActionPreference = "Stop"

$DefaultVersion = "__VERSION__"

$InstallDir = "$env:USERPROFILE\.local\bin"
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

$arch = if ([System.Environment]::Is64BitOperatingSystem) { "x86_64" } else {
    Write-Error "Unsupported architecture."
    exit 1
}
$target = "$arch-pc-windows-msvc"

$version = $env:CARRIER_VERSION
if (-not $version -and $DefaultVersion -ne "__VERSION__") {
    $version = $DefaultVersion
}

if ($version) {
    Write-Host "Fetching release $version..."
    $apiUrl = "https://api.github.com/repos/joshuamarie/carrier/releases/tags/$version"
} else {
    Write-Host "Fetching latest release..."
    $apiUrl = "https://api.github.com/repos/joshuamarie/carrier/releases/latest"
}

Write-Host "Fetching download URL for $target..."
$release = Invoke-RestMethod -Uri $apiUrl
$asset = $release.assets | Where-Object { $_.name -like "*$target.zip" } | Select-Object -First 1

if (-not $asset) {
    Write-Error "No release asset found for $target."
    exit 1
}

$tmp = New-Item -ItemType Directory -Path ([System.IO.Path]::GetTempPath()) -Name "carrier-install" -Force

Write-Host "Downloading carrier from $($asset.browser_download_url)..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile "$tmp\carrier.zip"
Expand-Archive -Path "$tmp\carrier.zip" -DestinationPath $tmp -Force

Move-Item -Path "$tmp\carrier.exe" -Destination "$InstallDir\carrier.exe" -Force
Remove-Item -Recurse -Force $tmp

Write-Host "carrier installed to $InstallDir\carrier.exe"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to your PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
    Write-Host "Please open a new terminal for the PATH change to take effect."
} else {
    Write-Host "$InstallDir is already in your PATH."
}
