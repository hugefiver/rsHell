# Import-safe: native initialization happens only at guarded query/child boundaries.
# One setup/restore lifecycle covers workspace and P0. The always caller must pass
# -Started $env:RSHELL_WORKSPACE_DISPLAY_STARTED along with Root/RunnerTemp/Ledger.
# Do not skip that caller merely because Root or an on-disk marker is missing.
# Workarea helper contract (owned separately): import-safe Get-WorkspaceWorkArea
# and Get-WorkspacePrimaryMonitorRect return exactly four signed Int32 coordinates;
# Set-WorkspaceWorkArea -Rect uses fWinIni=0 and returns an actual GET. The fixed
# workarea command lives in our child script, not in the import-only helper.
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "windows-display-child.ps1")

function ConvertTo-DisplayMode {
    param($Value)
    $fields = @('Width', 'Height', 'BitsPerPixel', 'Frequency')
    if ($null -eq $Value -or @($Value.PSObject.Properties).Count -ne 4) { throw "Display mode is invalid." }
    $mode = [ordered]@{}
    foreach ($field in $fields) {
        $property = $Value.PSObject.Properties[$field]
        if ($null -eq $property -or ($property.Value -isnot [int] -and $property.Value -isnot [long]) -or
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

function ConvertTo-DisplayRect {
    param($Value)
    if ($null -eq $Value -or @($Value.PSObject.Properties).Count -ne 4) { throw 'Display RECT is invalid.' }
    $rect = [ordered]@{}
    foreach ($field in @('Left', 'Top', 'Right', 'Bottom')) {
        if ($field -cnotin @($Value.PSObject.Properties.Name)) { throw 'Display RECT is invalid.' }
        $property = $Value.PSObject.Properties[$field]
        if ($null -eq $property -or ($property.Value -isnot [int] -and $property.Value -isnot [long]) -or
            $property.Value -lt [int]::MinValue -or $property.Value -gt [int]::MaxValue) { throw 'Display RECT is invalid.' }
        $rect[$field] = [int]$property.Value
    }
    if ($rect.Right -le $rect.Left -or $rect.Bottom -le $rect.Top) { throw 'Display RECT is invalid.' }
    return [pscustomobject]$rect
}

function Assert-DisplayRectEqual {
    param($Actual, $Expected)
    $actualRect = ConvertTo-DisplayRect $Actual
    $expectedRect = ConvertTo-DisplayRect $Expected
    foreach ($field in @('Left', 'Top', 'Right', 'Bottom')) {
        if ($actualRect.$field -ne $expectedRect.$field) { throw 'Display RECT did not match all four fields.' }
    }
}

function Write-DisplayRectObservation {
    param(
        [ValidateSet('baseline', 'target', 'child_applied', 'parent_after_exit', 'before_restore', 'restore_after_exit')][string]$Phase,
        [ValidateSet('setup', 'final', 'recovery', 'always')][string]$Arm, $Value
    )
    $rect = ConvertTo-DisplayRect $Value
    [Console]::WriteLine("RSHELL_WORKAREA phase=$Phase arm=$Arm flags=0 left=$($rect.Left) top=$($rect.Top) right=$($rect.Right) bottom=$($rect.Bottom)")
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

function Read-DisplayWorkAreaBaseline {
    param([string]$Ledger)
    Assert-DisplayPlainPath $Ledger
    return ConvertTo-DisplayRect (Get-Content -LiteralPath $Ledger -Raw -ErrorAction Stop | ConvertFrom-Json -ErrorAction Stop)
}

function Assert-WorkspaceDisplayHosted {
    param([string]$RunnerTemp)
    if (-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted') {
        throw 'Workspace display requires hosted Windows Actions.'
    }
    if ([string]::IsNullOrWhiteSpace($RunnerTemp) -or [string]::IsNullOrWhiteSpace($env:RUNNER_TEMP) -or
        -not (Test-Path -LiteralPath $RunnerTemp -PathType Container) -or
        [System.IO.Path]::GetFullPath($RunnerTemp).TrimEnd('\', '/') -cne [System.IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\', '/') -or
        [string]::IsNullOrWhiteSpace($env:GITHUB_ENV) -or -not (Test-Path -LiteralPath $env:GITHUB_ENV -PathType Leaf)) {
        throw 'Workspace display hosted environment is invalid.'
    }
    Assert-DisplayPlainPath $RunnerTemp
    Assert-DisplayPlainPath $env:GITHUB_ENV
}

function Initialize-WorkspaceDisplayNative {
    Assert-WorkspaceDisplayHosted $env:RUNNER_TEMP
    . (Join-Path $PSScriptRoot "windows-display-native.ps1")
    Initialize-RshellDisplayNative
}

function Get-WorkspaceDisplayWorkArea {
    Assert-WorkspaceDisplayHosted $env:RUNNER_TEMP
    . (Join-Path $PSScriptRoot 'windows-workarea.ps1')
    return ConvertTo-DisplayRect (Get-WorkspaceWorkArea)
}

function Get-WorkspaceDisplayPrimaryMonitorRect {
    Assert-WorkspaceDisplayHosted $env:RUNNER_TEMP
    . (Join-Path $PSScriptRoot 'windows-workarea.ps1')
    return ConvertTo-DisplayRect (Get-WorkspacePrimaryMonitorRect)
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
    Add-Content -LiteralPath $env:GITHUB_ENV -Value @("RSHELL_WORKSPACE_DISPLAY_ROOT=$Root", 'RSHELL_WORKSPACE_DISPLAY_STARTED=0') -ErrorAction Stop
}

function Publish-WorkspaceDisplayStarted {
    param([string]$Root)
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "RSHELL_WORKSPACE_DISPLAY_STARTED=$([System.IO.Path]::GetFileName($Root))" -ErrorAction Stop
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
    param([string]$Root, [string]$Ledger, $Baseline, $WorkAreaBaseline,
        [string]$Started = [System.IO.Path]::GetFileName($Root))
    if ($Ledger -cne (Join-Path $Root 'display-mode.json')) { throw "Display ledger ownership is inconsistent." }
    if ($Started -cne [System.IO.Path]::GetFileName($Root)) { throw 'Display started handoff is inconsistent.' }
    Assert-WorkspaceDisplayFiles $Root
    $markerPath = Join-Path $Root 'apply-started'
    Assert-DisplayPlainPath $markerPath
    if ([System.IO.File]::ReadAllText($markerPath) -cne $Started) { throw "Display start obligation is invalid." }
    if (Test-Path -LiteralPath (Join-Path $Root 'child-exit-unconfirmed')) { throw "Display child exit is unconfirmed; state retained." }
    Assert-DisplayEqual (Read-DisplayBaseline $Ledger) $Baseline
    Assert-DisplayRectEqual (Read-DisplayWorkAreaBaseline (Join-Path $Root 'workarea.json')) $WorkAreaBaseline
}

function Assert-WorkspaceDisplayFiles {
    param([string]$Root)
    Assert-DisplayPlainPath $Root
    foreach ($entry in [System.IO.Directory]::EnumerateFileSystemEntries($Root)) {
        if ([System.IO.Path]::GetFileName($entry) -cnotin @('display-mode.json', 'workarea.json', 'apply-started', 'child-exit-unconfirmed')) {
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
    # Only setup's explicit not-yet-mutated state or a validated Started=0 handoff
    # may enter this path. Marker absence alone is never preparation evidence.
    foreach ($name in @('display-mode.json', 'workarea.json', 'apply-started')) {
        Remove-WorkspaceDisplayFile (Join-Path $Root $name)
    }
    Remove-WorkspaceDisplayDirectory $Root
    if (Test-Path -LiteralPath $Root) { throw "Display preparation cleanup failed." }
}

function Remove-WorkspaceDisplayFile {
    param([string]$Path)
    [System.IO.File]::Delete($Path)
    if (Test-Path -LiteralPath $Path) { throw 'Display file cleanup failed.' }
}

function Remove-WorkspaceDisplayDirectory {
    param([string]$Root)
    [System.IO.Directory]::Delete($Root, $false)
}

function Clear-WorkspaceDisplayRestored {
    param([string]$Root, [string]$Ledger, $Baseline, $WorkAreaBaseline, [string]$Started)
    # Call only after BOTH mandatory post-restore reads match. Retire marker last;
    # partial deletion must retain a started obligation, never infer preparation.
    Assert-WorkspaceDisplayReady $Root $Ledger $Baseline $WorkAreaBaseline $Started
    try {
        foreach ($name in @('display-mode.json', 'workarea.json', 'apply-started')) {
            Remove-WorkspaceDisplayFile (Join-Path $Root $name)
        }
        Remove-WorkspaceDisplayDirectory $Root
        if (Test-Path -LiteralPath $Root) { throw 'Display restored cleanup failed.' }
    }
    catch {
        $failure = $_
        $marker = Join-Path $Root 'apply-started'
        if ((Test-Path -LiteralPath $Root -PathType Container) -and -not (Test-Path -LiteralPath $marker)) {
            try {
                Assert-WorkspaceDisplayFiles $Root
                Write-DisplayNewFile $marker $Started
            }
            catch {
                $failure.Exception.Data['RetirementFailure'] = $_
                [Console]::Error.WriteLine('RSHELL_DISPLAY obligation_retirement_failed state=retained')
            }
        }
        throw $failure
    }
}

function New-WorkspaceDisplayFailure {
    param([string]$Message, [object[]]$Failures)
    # Keep original ErrorRecords available to the caller, without interpolating
    # filesystem/native exception payloads into bounded production output.
    $exception = [System.InvalidOperationException]::new($Message)
    $exception.Data['Failures'] = $Failures
    return $exception
}

function Reset-WorkspaceDisplayBaseline {
    param([string]$Root, [string]$Ledger, $Baseline, $WorkAreaBaseline, [ValidateSet('between', 'recovery', 'always')][string]$Arm)
    Assert-WorkspaceDisplayReady $Root $Ledger $Baseline $WorkAreaBaseline
    $child = Invoke-WorkspaceDisplayChild $Root $Ledger 'Restore' 'Dynamic' $Baseline
    Write-DisplayObservation 'child_applied' $Arm 0 $child
    Assert-DisplayEqual $child $Baseline
    $current = Get-WorkspaceDisplayCurrent
    Write-DisplayObservation 'restore_after_exit' $Arm 0 $current
    Assert-DisplayEqual $current $Baseline
}

function Restore-WorkspaceDisplayBaselines {
    param([string]$Root, [string]$Ledger, $Baseline, $WorkAreaBaseline,
        [ValidateSet('recovery', 'always')][string]$Arm, [string]$Started)
    Assert-WorkspaceDisplayReady $Root $Ledger $Baseline $WorkAreaBaseline $Started
    $failures = [System.Collections.Generic.List[object]]::new()
    # Pre-restore observations are optional and independent, not prerequisites.
    try { Write-DisplayObservation 'before_restore' $Arm 0 (Get-WorkspaceDisplayCurrent) }
    catch { [Console]::WriteLine("RSHELL_DISPLAY phase=before_restore arm=$Arm flags=0 status=unavailable") }
    try { Write-DisplayRectObservation 'before_restore' $Arm (Get-WorkspaceDisplayWorkArea) }
    catch { [Console]::WriteLine("RSHELL_WORKAREA phase=before_restore arm=$Arm flags=0 status=unavailable") }
    try {
        $child = Invoke-WorkspaceDisplayChild $Root $Ledger 'Restore' 'Dynamic' $Baseline
        Write-DisplayObservation 'child_applied' $Arm 0 $child
        Assert-DisplayEqual $child $Baseline
    }
    catch { $failures.Add($_) }
    # No competing query/child/SET if exit or recovery ownership is uncertain.
    try { Assert-WorkspaceDisplayReady $Root $Ledger $Baseline $WorkAreaBaseline $Started }
    catch {
        $failures.Add($_)
        throw (New-WorkspaceDisplayFailure 'Display recovery unsafe; mode and workarea restoration incomplete; state retained.' $failures.ToArray())
    }
    try {
        $current = Get-WorkspaceDisplayCurrent
        Write-DisplayObservation 'restore_after_exit' $Arm 0 $current
        Assert-DisplayEqual $current $Baseline
    }
    catch { $failures.Add($_) }
    # Confirmed mode failure MUST NOT suppress the independently trusted RECT.
    try {
        $child = Invoke-WorkspaceDisplayChild $Root (Join-Path $Root 'workarea.json') 'Restore' 'Dynamic' $WorkAreaBaseline -Operation Workarea
        Write-DisplayRectObservation 'child_applied' $Arm $child
        Assert-DisplayRectEqual $child $WorkAreaBaseline
    }
    catch { $failures.Add($_) }
    try { Assert-WorkspaceDisplayReady $Root $Ledger $Baseline $WorkAreaBaseline $Started }
    catch {
        $failures.Add($_)
        throw (New-WorkspaceDisplayFailure 'Display recovery unsafe; workarea restoration incomplete; state retained.' $failures.ToArray())
    }
    try {
        $current = Get-WorkspaceDisplayWorkArea
        Write-DisplayRectObservation 'restore_after_exit' $Arm $current
        Assert-DisplayRectEqual $current $WorkAreaBaseline
    }
    catch { $failures.Add($_) }
    if ($failures.Count -ne 0) {
        throw (New-WorkspaceDisplayFailure 'Display mode or workarea restoration failed; state retained.' $failures.ToArray())
    }
}

function Invoke-WorkspaceDisplaySetup {
    param([string]$RunnerTemp, [int]$Width = 1920, [int]$Height = 1080)
    Assert-WorkspaceDisplayHosted $RunnerTemp
    if ($Width -le 0 -or $Height -le 0) { throw 'Display requested size is invalid.' }
    $displayRoot = Join-Path $RunnerTemp "rshell-workspace-display-$([Guid]::NewGuid().ToString('N'))"
    $displayRoot = Get-WorkspaceDisplayOwnedRoot $displayRoot $RunnerTemp
    $displayLedger = Join-Path $displayRoot 'display-mode.json'
    $created = $false
    $complete = $false
    $failure = $null
    $baseline = $null
    $workAreaBaseline = $null
    $obligationStarted = $false
    $started = [System.IO.Path]::GetFileName($displayRoot)
    $cleanupFailure = $null
    try {
        if (Test-Path -LiteralPath $displayRoot) { throw "Workspace display directory already exists." }
        [void](New-Item -ItemType Directory -Path $displayRoot -ErrorAction Stop)
        $created = $true
        Publish-WorkspaceDisplayRoot $displayRoot
        $baseline = Get-WorkspaceDisplayCurrent
        $baseline = ConvertTo-DisplayMode $baseline
        $workAreaBaseline = Get-WorkspaceDisplayWorkArea
        $workAreaBaseline = ConvertTo-DisplayRect $workAreaBaseline
        Write-DisplayNewFile $displayLedger ($baseline | ConvertTo-Json -Compress)
        Write-DisplayNewFile (Join-Path $displayRoot 'workarea.json') ($workAreaBaseline | ConvertTo-Json -Compress)
        Assert-DisplayEqual (Read-DisplayBaseline $displayLedger) $baseline
        Assert-DisplayRectEqual (Read-DisplayWorkAreaBaseline (Join-Path $displayRoot 'workarea.json')) $workAreaBaseline
        Write-DisplayObservation 'baseline' 'setup' 0 $baseline
        Write-DisplayRectObservation 'baseline' 'setup' $workAreaBaseline
        $target = Select-WorkspaceDisplayTarget $Width $Height
        $target = ConvertTo-DisplayMode $target
        if ($target.Width -lt $Width -or $target.Height -lt $Height) { throw "Display target is undersized." }
        Write-DisplayObservation 'target' 'setup' 0 $target
        if (($baseline | ConvertTo-Json -Compress) -ceq ($target | ConvertTo-Json -Compress)) {
            [Console]::WriteLine('RSHELL_DISPLAY discrimination=insufficient')
        }
        # Revalidate immutable originals after selection and before obligation.
        Assert-DisplayEqual (Read-DisplayBaseline $displayLedger) $baseline
        Assert-DisplayRectEqual (Read-DisplayWorkAreaBaseline (Join-Path $displayRoot 'workarea.json')) $workAreaBaseline
        Write-DisplayNewFile (Join-Path $displayRoot 'apply-started') $started
        Assert-WorkspaceDisplayReady $displayRoot $displayLedger $baseline $workAreaBaseline $started
        Publish-WorkspaceDisplayStarted $displayRoot
        $obligationStarted = $true
        foreach ($arm in @('fullscreen', 'dynamic', 'final')) {
            Assert-WorkspaceDisplayReady $displayRoot $displayLedger $baseline $workAreaBaseline $started
            $kind = if ($arm -eq 'fullscreen') { 'Fullscreen' } else { 'Dynamic' }
            $flags = if ($kind -eq 'Fullscreen') { 4 } else { 0 }
            $child = Invoke-WorkspaceDisplayChild $displayRoot $displayLedger 'ApplySelected' $kind $target
            Write-DisplayObservation 'child_applied' $arm $flags $child
            Assert-DisplayEqual $child $target
            $current = Get-WorkspaceDisplayCurrent
            Write-DisplayObservation 'parent_after_exit' $arm $flags $current
            if ($arm -eq 'fullscreen') { Reset-WorkspaceDisplayBaseline $displayRoot $displayLedger $baseline $workAreaBaseline 'between' }
            if ($arm -eq 'final') { Assert-DisplayEqual $current $target }
        }
        Assert-WorkspaceDisplayReady $displayRoot $displayLedger $baseline $workAreaBaseline $started
        $monitor = Get-WorkspaceDisplayPrimaryMonitorRect
        $monitor = ConvertTo-DisplayRect $monitor
        if ([long]$monitor.Right - $monitor.Left -lt $Width -or [long]$monitor.Bottom - $monitor.Top -lt $Height) {
            throw 'Primary monitor RECT is undersized.'
        }
        Write-DisplayRectObservation 'target' 'final' $monitor
        $child = Invoke-WorkspaceDisplayChild $displayRoot (Join-Path $displayRoot 'workarea.json') 'ApplySelected' 'Dynamic' $monitor -Operation Workarea
        Write-DisplayRectObservation 'child_applied' 'final' $child
        Assert-DisplayRectEqual $child $monitor
        Assert-WorkspaceDisplayReady $displayRoot $displayLedger $baseline $workAreaBaseline $started
        # Re-read both actuals before workspace/P0. SET success is not evidence.
        $current = Get-WorkspaceDisplayCurrent
        Write-DisplayObservation 'parent_after_exit' 'final' 0 $current
        Assert-DisplayEqual $current $target
        Assert-DisplayRectEqual (Get-WorkspaceDisplayPrimaryMonitorRect) $monitor
        $actual = Get-WorkspaceDisplayWorkArea
        Write-DisplayRectObservation 'parent_after_exit' 'final' $actual
        Assert-DisplayRectEqual $actual $monitor
        $complete = $true
    }
    catch { $failure = $_ }
    finally {
        if ($created -and -not $complete) {
            try {
                if ($obligationStarted) {
                    # Missing/corrupt baseline or uncertain exit refuses recovery and cleanup.
                    Restore-WorkspaceDisplayBaselines $displayRoot $displayLedger $baseline $workAreaBaseline 'recovery' $started
                    # Keep the published obligation and originals for the always
                    # caller even after successful immediate failure recovery.
                }
                else { Clear-WorkspaceDisplayPreparation $displayRoot }
            }
            catch {
                $cleanupFailure = $_
                [Console]::Error.WriteLine('RSHELL_DISPLAY setup_cleanup_failed state=retained')
            }
        }
    }
    if ($null -ne $failure) {
        $failures = @($failure)
        if ($null -ne $cleanupFailure) { $failures += $cleanupFailure }
        throw (New-WorkspaceDisplayFailure 'Workspace display setup failed.' $failures)
    }
}

function Invoke-WorkspaceDisplayRestore {
    param([string]$Root, [string]$RunnerTemp, [string]$Ledger,
        [string]$Started = $env:RSHELL_WORKSPACE_DISPLAY_STARTED)
    try {
        if ([string]::IsNullOrWhiteSpace($Root)) {
            if (-not [string]::IsNullOrWhiteSpace($Started) -or -not [string]::IsNullOrWhiteSpace($Ledger)) { throw 'Display root handoff is missing.' }
            return # Setup did not publish ANY handoff (e.g. an earlier CI step failed).
        }
        $ownedRoot = Get-WorkspaceDisplayOwnedRoot $Root $RunnerTemp
        if ($Ledger -cne (Join-Path $ownedRoot 'display-mode.json')) { throw "Display ledger ownership is inconsistent." }
        if ($Started -cne '0' -and $Started -cne [System.IO.Path]::GetFileName($ownedRoot)) { throw 'Display started handoff is missing or inconsistent.' }
        if (-not (Test-Path -LiteralPath $ownedRoot)) {
            if ($Started -ceq '0') { return } # Explicit, published no-mutation preparation.
            throw 'Started display root is missing; restoration unverified.'
        }
        if ($Started -ceq '0') {
            if (Test-Path -LiteralPath (Join-Path $ownedRoot 'apply-started')) { throw 'Display started handoff conflicts with obligation.' }
            Clear-WorkspaceDisplayPreparation $ownedRoot
            return
        }
        # Validate both immutable originals and obligation BEFORE any native boundary.
        $baseline = Read-DisplayBaseline $Ledger
        $workAreaBaseline = Read-DisplayWorkAreaBaseline (Join-Path $ownedRoot 'workarea.json')
        Assert-WorkspaceDisplayReady $ownedRoot $Ledger $baseline $workAreaBaseline $Started
        Assert-WorkspaceDisplayHosted $RunnerTemp
        Write-DisplayObservation 'baseline' 'always' 0 $baseline
        Write-DisplayRectObservation 'baseline' 'always' $workAreaBaseline
        Restore-WorkspaceDisplayBaselines $ownedRoot $Ledger $baseline $workAreaBaseline 'always' $Started
        Clear-WorkspaceDisplayRestored $ownedRoot $Ledger $baseline $workAreaBaseline $Started
        [Console]::WriteLine('RSHELL_DISPLAY cleanup=complete')
    }
    catch { throw (New-WorkspaceDisplayFailure 'Workspace display restoration failed; state retained.' @($_)) }
}
