# Unique parameter variable names avoid resetting a dot-sourcing caller's Mode or
# Ledger (windows-display.ps1 imports this file through the coordinator).
param(
    [Alias('Mode')][ValidateSet('ApplySelected', 'Restore')][string]$WorkareaChildMode,
    [Alias('Ledger')][string]$WorkareaChildLedger,
    [Alias('Left')][int]$WorkareaChildLeft,
    [Alias('Top')][int]$WorkareaChildTop,
    [Alias('Right')][int]$WorkareaChildRight,
    [Alias('Bottom')][int]$WorkareaChildBottom
)

# Only the fixed display/workarea helpers are supervised. Tests replace construction
# with a fixed, no-native sleeping fixture; no public mock/protocol switch exists.
# Workarea CLI: windows-display-child.ps1 -Mode ApplySelected|Restore -Ledger <original>
# -Left <Int32> -Top <Int32> -Right <Int32> -Bottom <Int32>. Both operations use
# fWinIni=0; Restore reads the immutable ledger. One actual GET line is required:
# RSHELL_WORKAREA_CHILD|RSHELL_WORKAREA_RESTORED left=N top=N right=N bottom=N flags=0
# (exactly one newline, no diagnostics/stdout payload; fixed error on stderr).
function New-WorkspaceDisplayChildStartInfo {
    param([string]$Ledger, [string]$Mode, [string]$ChangeKind, $Target,
        [ValidateSet('Display', 'Workarea')][string]$Operation = 'Display')
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = (Get-Command -Name pwsh -ErrorAction Stop).Source
    $arguments = if ($Operation -eq 'Workarea') {
        @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'windows-display-child.ps1'),
            '-Mode', $Mode, '-Ledger', $Ledger,
            '-Left', [string]$Target.Left, '-Top', [string]$Target.Top,
            '-Right', [string]$Target.Right, '-Bottom', [string]$Target.Bottom)
    }
    else {
        @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'windows-display.ps1'),
            '-Mode', $Mode, '-Ledger', $Ledger, '-ChangeKind', $ChangeKind,
            '-Width', [string]$Target.Width, '-Height', [string]$Target.Height,
            '-BitsPerPixel', [string]$Target.BitsPerPixel, '-Frequency', [string]$Target.Frequency)
    }
    foreach ($argument in $arguments) {
        $info.ArgumentList.Add($argument)
    }
    return $info
}

function Invoke-WorkspaceDisplayChild {
    param(
        [string]$Root, [string]$Ledger,
        [ValidateSet('ApplySelected', 'Restore')][string]$Mode,
        [ValidateSet('Fullscreen', 'Dynamic')][string]$ChangeKind, $Target,
        [ValidateRange(1, 120000)][int]$TimeoutMilliseconds = 120000,
        [ValidateRange(1, 5000)][int]$ReapMilliseconds = 5000,
        [ValidateSet('Display', 'Workarea')][string]$Operation = 'Display'
    )
    Assert-WorkspaceDisplayHosted $env:RUNNER_TEMP
    $ownedRoot = Get-WorkspaceDisplayOwnedRoot $Root $env:RUNNER_TEMP
    $modeLedger = Join-Path $ownedRoot 'display-mode.json'
    $areaLedger = Join-Path $ownedRoot 'workarea.json'
    if ($Ledger -cne $(if ($Operation -eq 'Workarea') { $areaLedger } else { $modeLedger })) { throw 'Display child ledger ownership is inconsistent.' }
    $baseline = Read-DisplayBaseline $modeLedger
    $areaBaseline = Read-DisplayWorkAreaBaseline $areaLedger
    Assert-WorkspaceDisplayReady $ownedRoot $modeLedger $baseline $areaBaseline
    if ($Operation -eq 'Workarea') {
        if ($ChangeKind -cne 'Dynamic') { throw 'Workarea requires flags=0.' }
        $Target = ConvertTo-DisplayRect $Target
        if ($Mode -eq 'Restore') { Assert-DisplayRectEqual $Target $areaBaseline }
    }
    else {
        $Target = ConvertTo-DisplayMode $Target
        if ($Mode -eq 'Restore') { Assert-DisplayEqual $Target $baseline }
    }
    $marker = Join-Path $Root 'child-exit-unconfirmed'
    $process = [System.Diagnostics.Process]::new()
    $confirmed = $false
    $attempted = $false
    $started = $false
    $marked = $false
    $failure = $null
    $cleanupFailure = $null
    $observation = $null
    try {
        $process.StartInfo = New-WorkspaceDisplayChildStartInfo $Ledger $Mode $ChangeKind $Target $Operation
        $process.StartInfo.UseShellExecute = $false
        $process.StartInfo.RedirectStandardOutput = $true
        $process.StartInfo.RedirectStandardError = $true
        Write-DisplayNewFile $marker '1'
        $marked = $true
        $attempted = $true
        try {
            $started = $process.Start()
        }
        catch [System.ComponentModel.Win32Exception] {
            # OS creation failed: no child exists. Other start errors remain uncertain.
            $confirmed = $true
            throw "Display child did not start."
        }
        catch {
            throw (New-WorkspaceDisplayFailure 'Display child start failed; exit unconfirmed; state retained.' @($_))
        }
        if (-not $started) { $confirmed = $true; throw "Display child did not start." }
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMilliseconds)) { throw "Display child timed out." }
        $confirmed = $true
        [Console]::WriteLine("RSHELL_DISPLAY child_exit=$($process.ExitCode)")
        if ($process.ExitCode -ne 0) { throw "Display child failed." }
        # This known helper emits one finite line and has no descendants. Even pipe
        # completion is bounded; never print raw child output or exception details.
        if (-not [System.Threading.Tasks.Task]::WaitAll([System.Threading.Tasks.Task[]]@($stdout, $stderr), $ReapMilliseconds)) {
            throw "Display child output did not complete."
        }
        if ($stderr.Result.Length -ne 0) { throw "Display child reported an error." }
        if ($Operation -eq 'Workarea') {
            $prefix = if ($Mode -eq 'Restore') { 'RSHELL_WORKAREA_RESTORED' } else { 'RSHELL_WORKAREA_CHILD' }
            $match = [regex]::Match($stdout.Result, "\A$prefix left=(-?[0-9]{1,10}) top=(-?[0-9]{1,10}) right=(-?[0-9]{1,10}) bottom=(-?[0-9]{1,10}) flags=0\r?\n\z")
            if (-not $match.Success) { throw 'Workarea child observation is invalid.' }
            $observation = ConvertTo-DisplayRect ([pscustomobject]@{
                Left = [long]$match.Groups[1].Value; Top = [long]$match.Groups[2].Value
                Right = [long]$match.Groups[3].Value; Bottom = [long]$match.Groups[4].Value
            })
        }
        else {
            $prefix = if ($Mode -eq 'Restore') { 'RSHELL_DISPLAY_RESTORED' } else { 'RSHELL_DISPLAY_CHILD' }
            $suffix = if ($Mode -eq 'Restore') { ' flags=0' } else { '' }
            $match = [regex]::Match($stdout.Result, "\A$prefix width=([0-9]{1,10}) height=([0-9]{1,10}) bpp=([0-9]{1,10}) frequency=([0-9]{1,10})$suffix\r?\n\z")
            if (-not $match.Success) { throw 'Display child observation is invalid.' }
            $observation = ConvertTo-DisplayMode ([pscustomobject]@{
                Width = [long]$match.Groups[1].Value; Height = [long]$match.Groups[2].Value
                BitsPerPixel = [long]$match.Groups[3].Value; Frequency = [long]$match.Groups[4].Value
            })
        }
    }
    catch { $failure = $_ }
    finally {
        if ($started -and -not $confirmed) {
            try {
                if (-not $process.HasExited) { $process.Kill() }
                $confirmed = $process.WaitForExit($ReapMilliseconds)
                if ($confirmed) { [Console]::WriteLine("RSHELL_DISPLAY child_reaped_exit=$($process.ExitCode)") }
            }
            catch { $confirmed = $false }
        }
        try {
            if ($marked -and ($confirmed -or -not $attempted)) {
                # A failed removal leaves the same authoritative uncertainty marker.
                Remove-WorkspaceDisplayFile $marker
                if (Test-Path -LiteralPath $marker) { throw "Display child exit marker retained." }
            }
        }
        catch { $cleanupFailure = $_ }
        finally { $process.Dispose() }
    }
    if ($null -ne $cleanupFailure) {
        $failures = @($cleanupFailure)
        if ($null -ne $failure) { $failures = @($failure, $cleanupFailure) }
        throw (New-WorkspaceDisplayFailure 'Display child failed; exit evidence retained.' $failures)
    }
    if ($null -ne $failure) { throw $failure }
    return $observation
}

function Invoke-WorkspaceWorkAreaChildOperation {
    param([ValidateSet('ApplySelected', 'Restore')][string]$Mode, [string]$Ledger, $Target)
    Assert-WorkspaceDisplayHosted $env:RUNNER_TEMP
    $root = Get-WorkspaceDisplayOwnedRoot ([System.IO.Path]::GetDirectoryName($Ledger)) $env:RUNNER_TEMP
    if ($Ledger -cne (Join-Path $root 'workarea.json')) { throw 'Workarea child ledger ownership is inconsistent.' }
    Assert-WorkspaceDisplayFiles $root
    $started = Join-Path $root 'apply-started'
    Assert-DisplayPlainPath $started
    if ([System.IO.File]::ReadAllText($started) -cne [System.IO.Path]::GetFileName($root)) { throw 'Workarea child obligation is invalid.' }
    # The parent's in-flight marker is required here, not cleared by this child.
    # The supervisor prevents another mutator from starting while it is present.
    $pending = Join-Path $root 'child-exit-unconfirmed'
    Assert-DisplayPlainPath $pending
    if ([System.IO.File]::ReadAllText($pending) -cne '1') { throw 'Workarea child ownership is invalid.' }
    [void](Read-DisplayBaseline (Join-Path $root 'display-mode.json'))
    $baseline = Read-DisplayWorkAreaBaseline $Ledger
    if ($Mode -eq 'Restore') { $Target = $baseline }
    else {
        $Target = ConvertTo-DisplayRect $Target
        Assert-DisplayRectEqual (Get-WorkspacePrimaryMonitorRect) $Target
    }
    $actual = ConvertTo-DisplayRect (Set-WorkspaceWorkArea -Rect $Target)
    Assert-DisplayRectEqual $actual $Target
    $prefix = if ($Mode -eq 'Restore') { 'RSHELL_WORKAREA_RESTORED' } else { 'RSHELL_WORKAREA_CHILD' }
    [Console]::WriteLine("$prefix left=$($actual.Left) top=$($actual.Top) right=$($actual.Right) bottom=$($actual.Bottom) flags=0")
}

# Importing is silent and native-free. Only the fixed child command imports the
# function-only workarea helper and enters its guarded setter/query boundaries.
if ($MyInvocation.InvocationName -ne '.') {
    $requestedMode = $WorkareaChildMode
    $requestedLedger = $WorkareaChildLedger
    $requestedRect = [pscustomobject]@{
        Left = $WorkareaChildLeft; Top = $WorkareaChildTop; Right = $WorkareaChildRight; Bottom = $WorkareaChildBottom
    }
    try {
        $ErrorActionPreference = 'Stop'
        Set-StrictMode -Version Latest
        . (Join-Path $PSScriptRoot 'windows-display-experiment.ps1')
        . (Join-Path $PSScriptRoot 'windows-workarea.ps1')
        if ([string]::IsNullOrWhiteSpace($requestedMode)) { throw 'Workarea child operation is missing.' }
        Invoke-WorkspaceWorkAreaChildOperation $requestedMode $requestedLedger $requestedRect
    }
    catch {
        [Console]::Error.WriteLine('RSHELL_WORKAREA helper_failed')
        exit 1
    }
}
