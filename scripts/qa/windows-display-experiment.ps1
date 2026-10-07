# Import-safe: native initialization happens only at the Current/selection boundaries.
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "windows-display-child.ps1")

function ConvertTo-DisplayMode {
    param($Value)
    $fields = @('Width', 'Height', 'BitsPerPixel', 'Frequency')
    if ($null -eq $Value -or @($Value.PSObject.Properties).Count -ne 4) { throw "Display mode is invalid." }
    $mode = [ordered]@{}
    foreach ($field in $fields) {
        $property = $Value.PSObject.Properties[$field]
        if ($null -eq $property -or $property.Value -isnot [int] -and $property.Value -isnot [long] -or
            $property.Value -lt 0 -or $property.Value -gt [int]::MaxValue -or
            ($field -ne 'Frequency' -and $property.Value -eq 0)) { throw "Display mode is invalid." }
        $mode[$field] = [int]$property.Value
    }
    return [pscustomobject]$mode
}

function Assert-DisplayEqual {
    param($Actual, $Expected)
    $actualMode = ConvertTo-DisplayMode $Actual
    $expectedMode = ConvertTo-DisplayMode $Expected
    foreach ($field in @('Width', 'Height', 'BitsPerPixel', 'Frequency')) {
        if ($actualMode.$field -ne $expectedMode.$field) { throw "Display mode did not match all four fields." }
    }
}

function Write-DisplayObservation {
    param(
        [ValidateSet('baseline', 'target', 'probe', 'child_applied', 'parent_after_exit', 'before_restore', 'restore_after_exit')][string]$Phase,
        [ValidateSet('p0', 'setup', 'fullscreen', 'dynamic', 'between', 'final', 'recovery', 'always')][string]$Arm,
        [ValidateSet(0, 4)][int]$Flags, $Value
    )
    $mode = ConvertTo-DisplayMode $Value
    [Console]::WriteLine("RSHELL_DISPLAY phase=$Phase arm=$Arm flags=$Flags width=$($mode.Width) height=$($mode.Height) bpp=$($mode.BitsPerPixel) frequency=$($mode.Frequency)")
}

function Write-DisplayNewFile {
    param([string]$Path, [string]$Text)
    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    try {
        $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Text)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally { $stream.Dispose() }
}

function Assert-DisplayPlainPath {
    param([string]$Path)
    if (([System.IO.File]::GetAttributes($Path) -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Display ownership path is a reparse point."
    }
}

function Read-DisplayBaseline {
    param([string]$Ledger)
    Assert-DisplayPlainPath $Ledger
    return ConvertTo-DisplayMode (Get-Content -LiteralPath $Ledger -Raw -ErrorAction Stop | ConvertFrom-Json -ErrorAction Stop)
}

function Initialize-WorkspaceDisplayNative {
    . (Join-Path $PSScriptRoot "windows-display-native.ps1")
    Initialize-RshellDisplayNative
}

function Get-WorkspaceDisplayCurrent {
    Initialize-WorkspaceDisplayNative
    return ConvertTo-DisplayMode ([RshellDisplayConfiguration]::Current())
}

function Select-WorkspaceDisplayTarget {
    param([int]$Width, [int]$Height)
    Initialize-WorkspaceDisplayNative
    return ConvertTo-DisplayMode ([RshellDisplayConfiguration]::PreferredAtLeast($Width, $Height))
}

function Publish-WorkspaceDisplayRoot {
    param([string]$Root)
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "RSHELL_WORKSPACE_DISPLAY_ROOT=$Root" -ErrorAction Stop
}

function Get-WorkspaceDisplayOwnedRoot {
    param([string]$Root, [string]$RunnerTemp)
    if (-not (Test-Path -LiteralPath $RunnerTemp -PathType Container)) { throw "Runner temporary directory is unavailable." }
    $runnerPath = [System.IO.Path]::GetFullPath($RunnerTemp).TrimEnd('\', '/')
    $ownedRoot = [System.IO.Path]::GetFullPath($Root)
    if ([System.IO.Path]::GetDirectoryName($ownedRoot) -ne $runnerPath -or
        [System.IO.Path]::GetFileName($ownedRoot) -cnotmatch '^rshell-workspace-display-[0-9a-f]{32}$') {
        throw "Workspace display path is not run-owned."
    }
    if (Test-Path -LiteralPath $ownedRoot) {
        if (-not (Test-Path -LiteralPath $ownedRoot -PathType Container)) { throw "Display root is not a directory." }
        Assert-DisplayPlainPath $ownedRoot
    }
    return $ownedRoot
}

function Assert-WorkspaceDisplayReady {
    param([string]$Root, [string]$Ledger, $Baseline)
    if ($Ledger -cne (Join-Path $Root 'display-mode.json')) { throw "Display ledger ownership is inconsistent." }
    Assert-WorkspaceDisplayFiles $Root
    $started = Join-Path $Root 'apply-started'
    Assert-DisplayPlainPath $started
    if ([System.IO.File]::ReadAllText($started) -cne '1') { throw "Display start obligation is invalid." }
    if (Test-Path -LiteralPath (Join-Path $Root 'child-exit-unconfirmed')) { throw "Display child exit is unconfirmed; state retained." }
    Assert-DisplayEqual (Read-DisplayBaseline $Ledger) $Baseline
}

function Assert-WorkspaceDisplayFiles {
    param([string]$Root)
    Assert-DisplayPlainPath $Root
    foreach ($entry in [System.IO.Directory]::EnumerateFileSystemEntries($Root)) {
        if ([System.IO.Path]::GetFileName($entry) -cnotin @('display-mode.json', 'apply-started', 'child-exit-unconfirmed')) {
            throw "Display cleanup has unknown state; retained."
        }
        Assert-DisplayPlainPath $entry
        if (-not (Test-Path -LiteralPath $entry -PathType Leaf)) { throw "Display cleanup has conflicting state; retained." }
    }
}

function Clear-WorkspaceDisplayPreparation {
    param([string]$Root)
    # Validate everything before deleting anything; never recurse or delete unknown files.
    Assert-WorkspaceDisplayFiles $Root
    if (Test-Path -LiteralPath (Join-Path $Root 'child-exit-unconfirmed')) { throw "Display child exit is unconfirmed; state retained." }
    # Retire the verified obligation first, keeping the baseline if retirement fails.
    foreach ($name in @('apply-started', 'display-mode.json')) {
        [System.IO.File]::Delete((Join-Path $Root $name))
    }
    [System.IO.Directory]::Delete($Root, $false)
    if (Test-Path -LiteralPath $Root) { throw "Display preparation cleanup failed." }
}

function Reset-WorkspaceDisplayBaseline {
    param([string]$Root, [string]$Ledger, $Baseline, [ValidateSet('between', 'recovery', 'always')][string]$Arm)
    Assert-WorkspaceDisplayReady $Root $Ledger $Baseline
    $child = Invoke-WorkspaceDisplayChild $Root $Ledger 'Restore' 'Dynamic' $Baseline
    Write-DisplayObservation 'child_applied' $Arm 0 $child
    Assert-DisplayEqual $child $Baseline
    $current = Get-WorkspaceDisplayCurrent
    Write-DisplayObservation 'restore_after_exit' $Arm 0 $current
    Assert-DisplayEqual $current $Baseline
}

function Invoke-WorkspaceDisplaySetup {
    param([string]$RunnerTemp, [int]$Width = 1920, [int]$Height = 1080)
    $displayRoot = Join-Path $RunnerTemp "rshell-workspace-display-$([Guid]::NewGuid().ToString('N'))"
    $displayRoot = Get-WorkspaceDisplayOwnedRoot $displayRoot $RunnerTemp
    $displayLedger = Join-Path $displayRoot 'display-mode.json'
    $created = $false
    $complete = $false
    $failure = $null
    $baseline = $null
    try {
        if (Test-Path -LiteralPath $displayRoot) { throw "Workspace display directory already exists." }
        [void](New-Item -ItemType Directory -Path $displayRoot -ErrorAction Stop)
        $created = $true
        Publish-WorkspaceDisplayRoot $displayRoot
        $baseline = Get-WorkspaceDisplayCurrent
        Write-DisplayNewFile $displayLedger ($baseline | ConvertTo-Json -Compress)
        Assert-DisplayEqual (Read-DisplayBaseline $displayLedger) $baseline
        Write-DisplayObservation 'baseline' 'setup' 0 $baseline
        $target = Select-WorkspaceDisplayTarget $Width $Height
        $target = ConvertTo-DisplayMode $target
        if ($target.Width -lt $Width -or $target.Height -lt $Height) { throw "Display target is undersized." }
        Write-DisplayObservation 'target' 'setup' 0 $target
        if (($baseline | ConvertTo-Json -Compress) -ceq ($target | ConvertTo-Json -Compress)) {
            [Console]::WriteLine('RSHELL_DISPLAY discrimination=insufficient')
        }
        Write-DisplayNewFile (Join-Path $displayRoot 'apply-started') '1'
        foreach ($arm in @('fullscreen', 'dynamic', 'final')) {
            Assert-WorkspaceDisplayReady $displayRoot $displayLedger $baseline
            $kind = if ($arm -eq 'fullscreen') { 'Fullscreen' } else { 'Dynamic' }
            $flags = if ($kind -eq 'Fullscreen') { 4 } else { 0 }
            $child = Invoke-WorkspaceDisplayChild $displayRoot $displayLedger 'ApplySelected' $kind $target
            Write-DisplayObservation 'child_applied' $arm $flags $child
            Assert-DisplayEqual $child $target
            $current = Get-WorkspaceDisplayCurrent
            Write-DisplayObservation 'parent_after_exit' $arm $flags $current
            if ($arm -eq 'fullscreen') { Reset-WorkspaceDisplayBaseline $displayRoot $displayLedger $baseline 'between' }
            if ($arm -eq 'final') { Assert-DisplayEqual $current $target }
        }
        $complete = $true
    }
    catch { $failure = $_ }
    finally {
        if ($created -and -not $complete) {
            try {
                if (Test-Path -LiteralPath (Join-Path $displayRoot 'apply-started')) {
                    # Missing/corrupt baseline or uncertain exit refuses recovery and cleanup.
                    Reset-WorkspaceDisplayBaseline $displayRoot $displayLedger $baseline 'recovery'
                }
                else { Clear-WorkspaceDisplayPreparation $displayRoot }
            }
            catch { [Console]::Error.WriteLine('RSHELL_DISPLAY setup_cleanup_failed state=retained') }
        }
    }
    if ($null -ne $failure) { throw $failure }
}

function Invoke-WorkspaceDisplayRestore {
    param([string]$Root, [string]$RunnerTemp, [string]$Ledger)
    if ([string]::IsNullOrWhiteSpace($Root)) { return }
    $ownedRoot = Get-WorkspaceDisplayOwnedRoot $Root $RunnerTemp
    if ($Ledger -cne (Join-Path $ownedRoot 'display-mode.json')) { throw "Display ledger ownership is inconsistent." }
    if (-not (Test-Path -LiteralPath $ownedRoot)) { return } # Setup's preparation finally already cleaned.
    if (-not (Test-Path -LiteralPath (Join-Path $ownedRoot 'apply-started'))) {
        Clear-WorkspaceDisplayPreparation $ownedRoot
        return
    }
    $baseline = Read-DisplayBaseline $Ledger
    Assert-WorkspaceDisplayReady $ownedRoot $Ledger $baseline
    Write-DisplayObservation 'baseline' 'always' 0 $baseline
    try {
        Write-DisplayObservation 'before_restore' 'always' 0 (Get-WorkspaceDisplayCurrent)
    }
    catch { [Console]::WriteLine('RSHELL_DISPLAY phase=before_restore arm=always flags=0 status=unavailable') }
    Reset-WorkspaceDisplayBaseline $ownedRoot $Ledger $baseline 'always'
    Clear-WorkspaceDisplayPreparation $ownedRoot
    [Console]::WriteLine('RSHELL_DISPLAY cleanup=complete')
}
