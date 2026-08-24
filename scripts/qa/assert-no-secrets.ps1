param(
    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string]$ArtifactRoot
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Test-BytePattern {
    param(
        [Parameter(Mandatory)]
        [AllowEmptyCollection()]
        [byte[]]$Bytes,

        [Parameter(Mandatory)]
        [byte[]]$Pattern
    )

    if ($Pattern.Length -eq 0 -or $Pattern.Length -gt $Bytes.Length) {
        return $false
    }

    $prefix = [int[]]::new($Pattern.Length)
    $matched = 0
    for ($index = 1; $index -lt $Pattern.Length; $index++) {
        while ($matched -gt 0 -and $Pattern[$matched] -ne $Pattern[$index]) {
            $matched = $prefix[$matched - 1]
        }

        if ($Pattern[$matched] -eq $Pattern[$index]) {
            $matched++
        }

        $prefix[$index] = $matched
    }

    $matched = 0
    foreach ($byte in $Bytes) {
        while ($matched -gt 0 -and $Pattern[$matched] -ne $byte) {
            $matched = $prefix[$matched - 1]
        }

        if ($Pattern[$matched] -eq $byte) {
            $matched++
        }

        if ($matched -eq $Pattern.Length) {
            return $true
        }
    }

    return $false
}

function Get-SecretMarkerNames {
    $names = [System.Collections.Generic.List[string]]::new()
    $configuredNames = [Environment]::GetEnvironmentVariable("RSHELL_QA_SECRET_ENV_VARS", "Process")

    if (-not [string]::IsNullOrWhiteSpace($configuredNames)) {
        foreach ($name in ($configuredNames -split "[,;`r`n]+")) {
            $trimmedName = $name.Trim()
            if (-not [string]::IsNullOrWhiteSpace($trimmedName)) {
                $names.Add($trimmedName)
            }
        }
    }

    foreach ($entry in Get-ChildItem Env:) {
        if (($entry.Name -like "RSHELL_QA_SECRET_*") -and
            ($entry.Name -ne "RSHELL_QA_SECRET_ENV_VARS")) {
            $names.Add($entry.Name)
        }

        if ($entry.Name -like "P0_SMOKE_SECRET_*") {
            $names.Add($entry.Name)
        }
    }

    return $names | Select-Object -Unique
}

function Get-RedactedArtifactPath {
    param(
        [Parameter(Mandatory)]
        [string]$Path,

        [Parameter(Mandatory)]
        [object[]]$Markers
    )

    $redactedPath = $Path
    foreach ($marker in $Markers) {
        $redactedPath = $redactedPath.Replace($marker.Value, "<redacted>")
    }

    return $redactedPath
}

if (-not (Test-Path -LiteralPath $ArtifactRoot -PathType Container)) {
    throw "Artifact root does not exist or is not a directory."
}

$artifactRootPath = (Resolve-Path -LiteralPath $ArtifactRoot).Path
$markers = [System.Collections.Generic.List[object]]::new()
foreach ($name in Get-SecretMarkerNames) {
    $value = [Environment]::GetEnvironmentVariable($name, "Process")
    if ($null -eq $value) {
        throw "Configured secret marker environment variable is unavailable."
    }

    if (-not [string]::IsNullOrEmpty($value)) {
        $markers.Add([pscustomobject]@{
                Name = $name
                Value = $value
                Encodings = @(
                    [System.Text.Encoding]::UTF8.GetBytes($value),
                    [System.Text.Encoding]::Unicode.GetBytes($value),
                    [System.Text.Encoding]::BigEndianUnicode.GetBytes($value)
                )
            })
    }
}

$findings = [System.Collections.Generic.List[string]]::new()
$files = @(Get-ChildItem -LiteralPath $artifactRootPath -File -Force -Recurse)
foreach ($file in $files) {
    try {
        $bytes = [System.IO.File]::ReadAllBytes($file.FullName)
    }
    catch {
        $relativePath = [System.IO.Path]::GetRelativePath($artifactRootPath, $file.FullName)
        $safePath = Get-RedactedArtifactPath -Path $relativePath -Markers $markers.ToArray()
        $findings.Add("could not scan artifact '$safePath'")
        continue
    }

    foreach ($marker in $markers) {
        foreach ($encoding in $marker.Encodings) {
            if (Test-BytePattern -Bytes $bytes -Pattern $encoding) {
                $relativePath = [System.IO.Path]::GetRelativePath($artifactRootPath, $file.FullName)
                $safePath = Get-RedactedArtifactPath -Path $relativePath -Markers $markers.ToArray()
                $findings.Add("secret marker from environment variable '$($marker.Name)' found in artifact '$safePath'")
                break
            }
        }
    }
}

if ($findings.Count -gt 0) {
    foreach ($finding in $findings) {
        [Console]::Error.WriteLine("assert-no-secrets: $finding")
    }

    exit 1
}

exit 0
