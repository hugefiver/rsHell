# Import-safe: only function definitions; no native initialization or work area mutation.
function ConvertTo-WorkspaceWorkAreaRect {
    param($Value)
    $fields = @('Left', 'Top', 'Right', 'Bottom')
    if ($null -eq $Value) { throw "Work area rectangle is invalid." }
    $dictionary = $Value -is [System.Collections.IDictionary]
    $names = if ($dictionary) { @($Value.Keys) } else { @($Value.PSObject.Properties.Name) }
    if ($names.Count -ne 4) { throw "Work area rectangle is invalid." }
    $rect = [ordered]@{}
    foreach ($field in $fields) {
        if ($field -cnotin $names) { throw "Work area rectangle is invalid." }
        # Assign directly so PowerShell cannot unwrap a singleton array into an accepted scalar.
        if ($dictionary) { $coordinate = $Value[$field] }
        else { $coordinate = $Value.PSObject.Properties[$field].Value }
        # JSON deserialization yields Int64; normalize only signed, in-range integral values.
        if (($coordinate -isnot [int] -and $coordinate -isnot [long]) -or
            $coordinate -lt [int]::MinValue -or $coordinate -gt [int]::MaxValue) {
            throw "Work area rectangle is invalid."
        }
        $rect[$field] = [int]$coordinate
    }
    if ($rect.Right -le $rect.Left -or $rect.Bottom -le $rect.Top) {
        throw "Work area rectangle must have positive extent."
    }
    return [pscustomobject]$rect
}

function Assert-WorkspaceWorkAreaHostedRunner {
    if (-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted') {
        throw "Work area changes require a GitHub-hosted Windows runner."
    }
}

function Initialize-WorkspaceWorkAreaNative {
    . (Join-Path $PSScriptRoot 'windows-display-native.ps1')
    Initialize-RshellDisplayNative
}

# Narrow replaceable native boundaries for no-native tests; no production mock switch.
function Get-WorkspaceWorkAreaNative {
    Initialize-WorkspaceWorkAreaNative
    return [RshellWorkAreaConfiguration]::GetWorkArea()
}

function Get-WorkspacePrimaryMonitorRectNative {
    Initialize-WorkspaceWorkAreaNative
    return [RshellWorkAreaConfiguration]::GetPrimaryMonitorRect()
}

function Set-WorkspaceWorkAreaNative {
    param($Rect)
    Assert-WorkspaceWorkAreaHostedRunner
    $validated = ConvertTo-WorkspaceWorkAreaRect $Rect
    Initialize-WorkspaceWorkAreaNative
    $nativeRect = [RshellWorkAreaRect]::new()
    foreach ($field in @('Left', 'Top', 'Right', 'Bottom')) { $nativeRect.$field = $validated.$field }
    [RshellWorkAreaConfiguration]::SetWorkArea($nativeRect)
}

function Get-WorkspaceWorkArea {
    return ConvertTo-WorkspaceWorkAreaRect (Get-WorkspaceWorkAreaNative)
}

function Get-WorkspacePrimaryMonitorRect {
    return ConvertTo-WorkspaceWorkAreaRect (Get-WorkspacePrimaryMonitorRectNative)
}

function Set-WorkspaceWorkArea {
    param([Parameter(Mandatory)]$Rect)
    # Guard before initialization or any native call, including the post-SET query.
    Assert-WorkspaceWorkAreaHostedRunner
    $validated = ConvertTo-WorkspaceWorkAreaRect $Rect
    [void](Set-WorkspaceWorkAreaNative -Rect $validated)
    # Return the real queried state, not the requested rectangle; caller verifies convergence.
    return Get-WorkspaceWorkArea
}
