param(
    [AllowEmptyString()][string]$Target = "",
    [AllowEmptyString()][string]$Package = "",
    [ValidateSet("", "invalid-target", "incomplete-report", "forbidden-binary-marker", "external-icon-payload", "external-resource-directory", "external-icons-directory", "runtime-icon-backends")]
    [string]$RegressionProbe = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-Throws {
    param([Parameter(Mandatory)][scriptblock]$Operation)

    try {
        & $Operation
    }
    catch {
        return
    }
    throw "Package regression probe accepted invalid input."
}

function Get-PackageContract {
    param([Parameter(Mandatory)][string]$BuildTarget)

    switch ($BuildTarget) {
        "x86_64-unknown-linux-gnu" {
            return [pscustomobject]@{ Format = "tar.gz"; Executable = "rshell"; Architecture = "elf-x86_64" }
        }
        "aarch64-apple-darwin" {
            return [pscustomobject]@{ Format = "tar.gz"; Executable = "rshell"; Architecture = "macho-arm64" }
        }
        "x86_64-pc-windows-msvc" {
            return [pscustomobject]@{ Format = "zip"; Executable = "rshell.exe"; Architecture = "pe-x86_64" }
        }
        default { throw "Unsupported release target." }
    }
}

function Assert-StartupReport {
    param([Parameter(Mandatory)]$Report)

    foreach ($propertyName in @(
            "window_realized",
            "local_session_connected",
            "non_empty_render_frame",
            "shutdown_clean",
            "embedded_css_loaded",
            "embedded_icons_renderable"
        )) {
        $property = $Report.PSObject.Properties[$propertyName]
        if ($null -eq $property -or $property.Value -isnot [bool] -or -not $property.Value) {
            throw "Startup smoke report is incomplete."
        }
    }
    $backend = $Report.PSObject.Properties["embedded_icon_backend"]
    if ($null -eq $backend -or $backend.Value -notin @("gtk_svg", "internal_vector")) {
        throw "Startup smoke report has an invalid embedded icon backend."
    }
}

function Assert-ExecutableArchitecture {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Architecture
    )

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64) {
        throw "Packaged executable is truncated."
    }

    switch ($Architecture) {
        "elf-x86_64" {
            if ($bytes[0] -ne 0x7f -or $bytes[1] -ne 0x45 -or $bytes[2] -ne 0x4c -or $bytes[3] -ne 0x46 -or
                $bytes[4] -ne 2 -or $bytes[5] -ne 1 -or [System.BitConverter]::ToUInt16($bytes, 18) -ne 62) {
                throw "Packaged executable architecture does not match the release target."
            }
        }
        "macho-arm64" {
            if ($bytes[0] -ne 0xcf -or $bytes[1] -ne 0xfa -or $bytes[2] -ne 0xed -or $bytes[3] -ne 0xfe -or
                [System.BitConverter]::ToUInt32($bytes, 4) -ne 0x0100000c) {
                throw "Packaged executable architecture does not match the release target."
            }
        }
        "pe-x86_64" {
            if ($bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
                throw "Packaged executable architecture does not match the release target."
            }
            $headerOffset = [System.BitConverter]::ToInt32($bytes, 0x3c)
            if ($headerOffset -lt 0 -or $headerOffset -gt ($bytes.Length - 6) -or
                $bytes[$headerOffset] -ne 0x50 -or $bytes[$headerOffset + 1] -ne 0x45 -or
                $bytes[$headerOffset + 2] -ne 0 -or $bytes[$headerOffset + 3] -ne 0 -or
                [System.BitConverter]::ToUInt16($bytes, $headerOffset + 4) -ne 0x8664) {
                throw "Packaged executable architecture does not match the release target."
            }
        }
        default { throw "Package architecture contract is invalid." }
    }
}

function Assert-NoForbiddenExecutableMarker {
    param([Parameter(Mandatory)][string]$Path)

    $contents = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($Path))
    foreach ($marker in @(
            "libssh2", "ssh2.dll", "ssh2.so", "ssh2.dylib", "libssl", "libcrypto",
            "wezterm-ssh", "wezterm_ssh", "05343b"
        )) {
        if ($contents.IndexOf($marker, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "Packaged executable contains a forbidden binary marker."
        }
    }
}

function Assert-PayloadFile {
    param(
        [Parameter(Mandatory)][string]$PayloadRoot,
        [Parameter(Mandatory)][string]$RelativePath
    )

    $path = Join-Path $PayloadRoot $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or (Get-Item -LiteralPath $path).Length -eq 0) {
        throw "Required package runtime file is missing."
    }
    return $path
}

function Assert-PayloadDirectory {
    param(
        [Parameter(Mandatory)][string]$PayloadRoot,
        [Parameter(Mandatory)][string]$RelativePath
    )

    $path = Join-Path $PayloadRoot $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Container)) {
        throw "Required package runtime directory is missing."
    }
    return $path
}

function Assert-NoLegacyArtifact {
    param([Parameter(Mandatory)][string]$PayloadRoot)

    foreach ($entry in Get-ChildItem -LiteralPath $PayloadRoot -Force -Recurse) {
        if (($entry.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Package contains a reparse point."
        }
        $relative = [System.IO.Path]::GetRelativePath($PayloadRoot, $entry.FullName)
        if ($relative -match "(?i)(^|[\\/])(?:lib)?(?:ssh2|ssl|crypto)|wezterm|05343b") {
            throw "Package contains a removed SSH engine artifact."
        }
    }
}

function Assert-NoProductAssetPayload {
    param([Parameter(Mandatory)][string]$PayloadRoot)

    foreach ($entry in Get-ChildItem -LiteralPath $PayloadRoot -Force -Recurse) {
        $relative = [System.IO.Path]::GetRelativePath($PayloadRoot, $entry.FullName)
        if ($relative -match "(?i)(^|[\\/])(resources|icons)([\\/]|$)" -or
            (-not $entry.PSIsContainer -and $entry.Extension -ieq ".svg")) {
            throw "Package contains an external rsHell icon/resource payload."
        }
    }
}

function Expand-Package {
    param(
        [Parameter(Mandatory)][string]$ArchivePath,
        [Parameter(Mandatory)][string]$Destination,
        [Parameter(Mandatory)][string]$Format
    )

    if ($Format -eq "zip") {
        [System.IO.Compression.ZipFile]::ExtractToDirectory($ArchivePath, $Destination, $false)
        return
    }

    $archive = [System.IO.File]::OpenRead($ArchivePath)
    try {
        $gzip = [System.IO.Compression.GZipStream]::new($archive, [System.IO.Compression.CompressionMode]::Decompress, $false)
        try {
            [System.Formats.Tar.TarFile]::ExtractToDirectory($gzip, $Destination, $false)
        }
        finally {
            $gzip.Dispose()
        }
    }
    finally {
        $archive.Dispose()
    }
}

function Remove-UniqueTempDirectory {
    param([Parameter(Mandatory)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        return
    }
    $temporaryBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if (-not $temporaryBase.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
        $temporaryBase += [System.IO.Path]::DirectorySeparatorChar
    }
    $resolved = [System.IO.Path]::GetFullPath($Path)
    $leaf = [System.IO.Path]::GetFileName($resolved.TrimEnd([System.IO.Path]::DirectorySeparatorChar))
    if (-not $resolved.StartsWith($temporaryBase, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not $leaf.StartsWith("rshell-package-", [System.StringComparison]::Ordinal)) {
        throw "Package cleanup target failed its safety check."
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
    if (Test-Path -LiteralPath $resolved) {
        throw "Package temporary directory cleanup failed."
    }
}

function Invoke-StartupSmoke {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [Parameter(Mandatory)][string]$WorkingDirectory,
        [Parameter(Mandatory)][string]$ReportPath
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $Executable
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.ArgumentList.Add("--smoke-startup")
    $startInfo.ArgumentList.Add($ReportPath)

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $started = $false
    try {
        if (-not $process.Start()) {
            throw "Packaged startup smoke could not start."
        }
        $started = $true
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(120000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw "Packaged startup smoke timed out."
        }
        [void]$stdout.GetAwaiter().GetResult()
        [void]$stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) {
            throw "Packaged startup smoke failed."
        }
    }
    finally {
        if ($started -and -not $process.HasExited) {
            try { $process.Kill($true) } catch {}
            $process.WaitForExit()
        }
        $process.Dispose()
    }
}

if ($RegressionProbe) {
    switch ($RegressionProbe) {
        "invalid-target" {
            Assert-Throws { Get-PackageContract -BuildTarget "invalid-target" }
        }
        "incomplete-report" {
            $report = '{"window_realized":true,"local_session_connected":true,"non_empty_render_frame":true,"shutdown_clean":false}' | ConvertFrom-Json
            Assert-Throws { Assert-StartupReport -Report $report }
        }
        "forbidden-binary-marker" {
            $temporaryRoot = [System.IO.Path]::GetTempPath()
            if (-not (Test-Path -LiteralPath $temporaryRoot -PathType Container)) {
                throw "Package regression probe temporary directory is unavailable."
            }
            $probePath = Join-Path $temporaryRoot "rshell-package-marker-$([Guid]::NewGuid().ToString('N')).bin"
            $probeBytes = [byte[]]::new(128)
            $probeBytes[0] = 0x4d
            $probeBytes[1] = 0x5a
            [System.BitConverter]::GetBytes(64).CopyTo($probeBytes, 0x3c)
            $probeBytes[64] = 0x50
            $probeBytes[65] = 0x45
            $probeBytes[68] = 0x64
            $probeBytes[69] = 0x86
            [System.Text.Encoding]::ASCII.GetBytes("libssh2.dll").CopyTo($probeBytes, 80)
            try {
                [System.IO.File]::WriteAllBytes($probePath, $probeBytes)
                Assert-ExecutableArchitecture -Path $probePath -Architecture "pe-x86_64"
                Assert-Throws { Assert-NoForbiddenExecutableMarker -Path $probePath }
            }
            finally {
                if (Test-Path -LiteralPath $probePath -PathType Leaf) {
                    Remove-Item -LiteralPath $probePath -Force
                }
                if (Test-Path -LiteralPath $probePath) {
                    throw "Package regression probe cleanup failed."
                }
            }
        }
        "external-icon-payload" {
            $probeRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rshell-package-assets-$([Guid]::NewGuid().ToString('N'))"
            $iconRoot = Join-Path (Join-Path $probeRoot "resources") "icons"
            [void](New-Item -ItemType Directory -Path $iconRoot -Force)
            try {
                [System.IO.File]::WriteAllText((Join-Path $iconRoot "import.svg"), "<svg/>")
                Assert-Throws { Assert-NoProductAssetPayload -PayloadRoot $probeRoot }
            }
            finally {
                Remove-UniqueTempDirectory -Path $probeRoot
            }
        }
        "external-resource-directory" {
            $probeRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rshell-package-resource-$([Guid]::NewGuid().ToString('N'))"
            $assetRoot = Join-Path (Join-Path $probeRoot "nested") "resources"
            [void](New-Item -ItemType Directory -Path $assetRoot -Force)
            try {
                [System.IO.File]::WriteAllText((Join-Path $assetRoot "payload.css"), "payload")
                Assert-Throws { Assert-NoProductAssetPayload -PayloadRoot $probeRoot }
            }
            finally {
                Remove-UniqueTempDirectory -Path $probeRoot
            }
        }
        "external-icons-directory" {
            $probeRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rshell-package-icons-$([Guid]::NewGuid().ToString('N'))"
            $assetRoot = Join-Path (Join-Path $probeRoot "nested") "icons"
            [void](New-Item -ItemType Directory -Path $assetRoot -Force)
            try {
                [System.IO.File]::WriteAllBytes((Join-Path $assetRoot "payload.png"), [byte[]](0x89, 0x50, 0x4e, 0x47))
                Assert-Throws { Assert-NoProductAssetPayload -PayloadRoot $probeRoot }
            }
            finally {
                Remove-UniqueTempDirectory -Path $probeRoot
            }
        }
        "runtime-icon-backends" {
            foreach ($backend in @("gtk_svg", "internal_vector")) {
                $report = [pscustomobject]@{
                    window_realized = $true; local_session_connected = $true
                    non_empty_render_frame = $true; shutdown_clean = $true
                    embedded_css_loaded = $true; embedded_icons_renderable = $true
                    embedded_icon_backend = $backend
                }
                Assert-StartupReport -Report $report
            }
            $invalid = [pscustomobject]@{
                window_realized = $true; local_session_connected = $true
                non_empty_render_frame = $true; shutdown_clean = $true
                embedded_css_loaded = $true; embedded_icons_renderable = $true
                embedded_icon_backend = "theme_or_file_fallback"
            }
            Assert-Throws { Assert-StartupReport -Report $invalid }
        }
    }
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Target) -or [string]::IsNullOrWhiteSpace($Package)) {
    throw "Target and package are required."
}

$contract = Get-PackageContract -BuildTarget $Target
if (-not (Test-Path -LiteralPath $Package -PathType Leaf)) {
    throw "Package archive is missing."
}
$packagePath = (Resolve-Path -LiteralPath $Package).Path
if ((Get-Item -LiteralPath $packagePath).Length -eq 0) {
    throw "Package archive is empty."
}
switch ($contract.Format) {
    "zip" {
        if (-not $packagePath.EndsWith(".zip", [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Package archive format does not match the release target."
        }
    }
    "tar.gz" {
        if (-not $packagePath.EndsWith(".tar.gz", [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Package archive format does not match the release target."
        }
    }
    default { throw "Package format contract is invalid." }
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rshell-package-$([Guid]::NewGuid().ToString('N'))"
[void](New-Item -ItemType Directory -Path $temporaryRoot)
try {
    Expand-Package -ArchivePath $packagePath -Destination $temporaryRoot -Format $contract.Format
    $rootDirectories = @(Get-ChildItem -LiteralPath $temporaryRoot -Directory -Force)
    $rootFiles = @(Get-ChildItem -LiteralPath $temporaryRoot -File -Force)
    if ($rootDirectories.Count -ne 1 -or $rootFiles.Count -ne 0) {
        throw "Package archive has an invalid root layout."
    }
    $payloadRoot = $rootDirectories[0].FullName
    if ($rootDirectories[0].Name -cne "rshell-$Target") {
        throw "Package archive root does not match the release target."
    }
    Assert-NoLegacyArtifact -PayloadRoot $payloadRoot
    Assert-NoProductAssetPayload -PayloadRoot $payloadRoot

    $executable = Assert-PayloadFile -PayloadRoot $payloadRoot -RelativePath $contract.Executable
    Assert-ExecutableArchitecture -Path $executable -Architecture $contract.Architecture
    Assert-NoForbiddenExecutableMarker -Path $executable

    if ($Target -in @("x86_64-unknown-linux-gnu", "aarch64-apple-darwin")) {
        [void](Assert-PayloadFile -PayloadRoot $payloadRoot -RelativePath "LICENSE")
        [void](Assert-PayloadFile -PayloadRoot $payloadRoot -RelativePath "README.md")
    }
    else {
        foreach ($runtimeFile in @(
                "gtk-4-1.dll",
                "glib-2.0-0.dll",
                "gobject-2.0-0.dll",
                "gio-2.0-0.dll",
                "gdk_pixbuf-2.0-0.dll",
                "pango-1.0-0.dll",
                "fontconfig-1.dll",
                "share\glib-2.0\schemas\gschemas.compiled",
                "etc\fonts\fonts.conf"
            )) {
            [void](Assert-PayloadFile -PayloadRoot $payloadRoot -RelativePath $runtimeFile)
        }
        $pixbufRoot = Assert-PayloadDirectory -PayloadRoot $payloadRoot -RelativePath "lib\gdk-pixbuf-2.0"
        $pixbufRuntime = @(Get-ChildItem -LiteralPath $pixbufRoot -Directory -Force)
        if ($pixbufRuntime.Count -ne 1 -or $pixbufRuntime[0].Name -cne "2.10.0") {
            throw "GDK Pixbuf runtime layout is invalid."
        }
        $loaderCache = Assert-PayloadFile -PayloadRoot $payloadRoot -RelativePath "lib\gdk-pixbuf-2.0\2.10.0\loaders.cache"
        $loaderDirectory = Join-Path $pixbufRuntime[0].FullName "loaders"
        if (Test-Path -LiteralPath $loaderDirectory -PathType Container) {
            $loaderLibraries = @(Get-ChildItem -LiteralPath $loaderDirectory -Filter "*.dll" -File)
            if ($loaderLibraries.Count -eq 0) {
                throw "Required package runtime file is missing."
            }
            if ([System.IO.File]::ReadAllText($loaderCache) -match '(?m)^"[A-Za-z]:[\\/]') {
                throw "GDK Pixbuf loader cache is not relocatable."
            }
        }
        elseif ([System.IO.File]::ReadAllText($loaderCache) -notmatch "GdkPixbuf Image Loader Modules file") {
            throw "GDK Pixbuf built-in loader cache is invalid."
        }
        $fontconfigRoot = Assert-PayloadDirectory -PayloadRoot $payloadRoot -RelativePath "share\fontconfig"
        if (@(Get-ChildItem -LiteralPath $fontconfigRoot -File -Force -Recurse).Count -eq 0) {
            throw "Required package runtime file is missing."
        }
    }

    $originalEnvironment = @{}
    foreach ($name in @("GSETTINGS_SCHEMA_DIR", "GDK_PIXBUF_MODULE_FILE", "FONTCONFIG_FILE", "FONTCONFIG_PATH")) {
        $originalEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, "Process")
    }
    try {
        if ($Target -eq "x86_64-pc-windows-msvc") {
            $env:GSETTINGS_SCHEMA_DIR = Join-Path $payloadRoot "share\glib-2.0\schemas"
            $env:GDK_PIXBUF_MODULE_FILE = $loaderCache
            $env:FONTCONFIG_FILE = Join-Path $payloadRoot "etc\fonts\fonts.conf"
            $env:FONTCONFIG_PATH = Join-Path $payloadRoot "etc\fonts"
        }
        $reportPath = Join-Path $temporaryRoot "startup-report.json"
        Invoke-StartupSmoke -Executable $executable -WorkingDirectory $payloadRoot -ReportPath $reportPath
        if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
            throw "Packaged startup smoke did not write a report."
        }
        $report = [System.IO.File]::ReadAllText($reportPath) | ConvertFrom-Json
        Assert-StartupReport -Report $report
    }
    finally {
        foreach ($entry in $originalEnvironment.GetEnumerator()) {
            [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, "Process")
        }
    }
}
finally {
    Remove-UniqueTempDirectory -Path $temporaryRoot
}

exit 0
