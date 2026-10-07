$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'windows-display-experiment.ps1')

function Assert-Test {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw "Display no-native test: $Message" }
}

function Expect-Failure {
    param([scriptblock]$Action, [string]$Message)
    $failure = $null
    try { & $Action } catch { $failure = $_ }
    Assert-Test ($null -ne $failure) 'expected failure was accepted'
    if ($Message.Length -gt 0) {
        Assert-Test ($failure.Exception.Message.Contains($Message)) "wrong failure: $($failure.Exception.Message)"
    }
}

$script:baseline = [pscustomobject]@{ Width = 1024; Height = 768; BitsPerPixel = 32; Frequency = 60 }
$script:target = [pscustomobject]@{ Width = 1920; Height = 1080; BitsPerPixel = 32; Frequency = 60 }
$script:realWrite = ${function:Write-DisplayNewFile}
$script:realClear = ${function:Clear-WorkspaceDisplayPreparation}
$script:realChild = ${function:Invoke-WorkspaceDisplayChild}
$script:roots = [System.Collections.Generic.List[string]]::new()
$script:events = [System.Collections.Generic.List[string]]::new()
$script:applications = [System.Collections.Generic.List[string]]::new()
$script:root = ''
$script:fault = 'none'
$script:reads = 0
$script:selections = 0
$script:current = $script:baseline
$fixtureRoot = ''

# None of these seams imports the native helper, loads Add-Type, or calls P/Invoke.
function Initialize-WorkspaceDisplayNative { throw 'Native initialization is forbidden in this test.' }
function Publish-WorkspaceDisplayRoot {
    param($Root)
    $script:root = $Root
    $script:roots.Add($Root) # Own the exact fixture path even if publication fails.
    $script:events.Add('publish')
    Assert-Test (-not (Test-Path -LiteralPath (Join-Path $Root 'display-mode.json'))) 'baseline preceded publication'
    if ($script:fault -eq 'publication') { throw 'publication_failure' }
}
function Get-WorkspaceDisplayCurrent {
    $script:reads++
    $script:events.Add('current')
    if ($script:fault -eq 'capture' -and $script:reads -eq 1) { throw 'capture_failure' }
    if ($script:fault -eq 'before-restore-current' -and $script:reads -eq 1) { throw 'optional_current_failure' }
    return $script:current
}
function Select-WorkspaceDisplayTarget {
    param($Width, $Height)
    $script:selections++
    $script:events.Add('select')
    Assert-Test ($Width -eq 1920 -and $Height -eq 1080) 'minimum target weakened'
    Assert-DisplayEqual (Read-DisplayBaseline (Join-Path $script:root 'display-mode.json')) $script:baseline
    Assert-Test (-not (Test-Path -LiteralPath (Join-Path $script:root 'apply-started'))) 'obligation preceded selection'
    return $script:target
}
function Write-DisplayNewFile {
    param($Path, $Text)
    if ($script:fault -eq 'partial-baseline' -and [System.IO.Path]::GetFileName($Path) -eq 'display-mode.json') {
        & $script:realWrite $Path '{'
        throw 'baseline_write_failure'
    }
    & $script:realWrite $Path $Text
}
function Clear-WorkspaceDisplayPreparation {
    param($Root)
    if ($script:fault -eq 'cleanup') { throw 'cleanup_failure' }
    & $script:realClear $Root
}
function Invoke-WorkspaceDisplayChild {
    param($Root, $Ledger, $Mode, $ChangeKind, $Target)
    Assert-WorkspaceDisplayReady $Root $Ledger $script:baseline
    $script:events.Add("$Mode/$ChangeKind")
    if ($Mode -eq 'ApplySelected') {
        Assert-DisplayEqual $Target $script:target
        $script:applications.Add(($Target | ConvertTo-Json -Compress))
        $script:current = $Target
        if ($script:fault -eq 'child-failure') { throw 'child_failure' }
        if ($script:fault -eq 'lost-ledger') { [System.IO.File]::Delete($Ledger); throw 'lost_ledger' }
        if ($script:fault -eq 'corrupt-ledger') { [System.IO.File]::WriteAllText($Ledger, '{'); throw 'corrupt_ledger' }
        if ($script:fault -eq 'uncertain') {
            Write-DisplayNewFile (Join-Path $Root 'child-exit-unconfirmed') '1'
            throw 'uncertain_child'
        }
        if ($script:fault -eq 'child-mismatch') {
            return [pscustomobject]@{ Width = 1920; Height = 1080; BitsPerPixel = 16; Frequency = 60 }
        }
        if ($ChangeKind -eq 'Fullscreen') { $script:current = $script:baseline } # Observed exit rollback is not failure.
        if ($script:fault -eq 'final-mismatch' -and $script:applications.Count -eq 3) {
            $script:current = [pscustomobject]@{ Width = 1920; Height = 1080; BitsPerPixel = 32; Frequency = 59 }
        }
    }
    else {
        Assert-Test ($ChangeKind -eq 'Dynamic') 'restore used fullscreen'
        Assert-DisplayEqual $Target $script:baseline
        if ($script:fault -eq 'restore-child-failure') { throw 'restore_child_failure' }
        $script:current = $Target
        if ($script:fault -eq 'reset-mismatch' -or $script:fault -eq 'restore-mismatch') {
            $script:current = [pscustomobject]@{ Width = 1024; Height = 768; BitsPerPixel = 16; Frequency = 60 }
        }
    }
    return $Target
}

function Reset-TestCase {
    param([string]$Fault = 'none')
    $script:fault = $Fault
    $script:events.Clear()
    $script:applications.Clear()
    $script:reads = 0
    $script:selections = 0
    $script:current = $script:baseline
}
function Invoke-TestRestore {
    Invoke-WorkspaceDisplayRestore $script:root $script:tempRoot (Join-Path $script:root 'display-mode.json')
}

$tempParent = [System.IO.Path]::GetTempPath()
Assert-Test (Test-Path -LiteralPath $tempParent -PathType Container) 'temporary parent missing'
# Owned before creation; the finally deletes only exact known fixture paths, nonrecursively.
$script:tempRoot = Join-Path $tempParent "rshell-display-test-$([Guid]::NewGuid().ToString('N'))"
Assert-Test (-not (Test-Path -LiteralPath $script:tempRoot)) 'fixture already exists'
try {
    [void](New-Item -ItemType Directory -Path $script:tempRoot)
    Assert-Test (-not ('RshellDisplayConfiguration' -as [type])) 'native type loaded before test'
    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    Assert-Test (($script:events -join ',') -ceq 'publish,current,select,ApplySelected/Fullscreen,current,Restore/Dynamic,current,ApplySelected/Dynamic,current,ApplySelected/Dynamic,current') 'arm/reset/final ordering changed'
    Assert-Test ($script:selections -eq 1 -and $script:applications.Count -eq 3) 'target selection was repeated'
    Assert-Test (@($script:applications | Select-Object -Unique).Count -eq 1) 'targets differ between arms'
    $ledger = Join-Path $script:root 'display-mode.json'
    $savedBytes = [System.IO.File]::ReadAllText($ledger)
    Expect-Failure { Write-DisplayNewFile $ledger '{}' } ''
    Assert-Test ([System.IO.File]::ReadAllText($ledger) -ceq $savedBytes) 'baseline was overwritten'
    Invoke-TestRestore
    Assert-Test (-not (Test-Path -LiteralPath $script:root)) 'always restore did not clean'
    Assert-Test (($script:events -join ',').EndsWith('current,Restore/Dynamic,current')) 'always restore lacked parent Current'

    # An unavailable optional first Current cannot suppress the required flags-0
    # child, mandatory post-child Current/baseline verification, or safe cleanup.
    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    Reset-TestCase 'before-restore-current'
    $restoreFailure = $null
    $restoreLog = [System.IO.StringWriter]::new()
    $consoleOutput = [Console]::Out
    try {
        [Console]::SetOut($restoreLog)
        try { Invoke-TestRestore } catch { $restoreFailure = $_ }
    }
    finally {
        [Console]::SetOut($consoleOutput)
        $restoreOutput = $restoreLog.ToString()
        $restoreLog.Dispose()
    }
    Assert-Test ($script:events.Contains('Restore/Dynamic')) 'optional pre-Restore Current failure skipped flags0 Restore child'
    Assert-Test ($null -eq $restoreFailure) 'optional pre-Restore Current failure prevented restoration'
    Assert-Test (($script:events -join ',') -ceq 'current,Restore/Dynamic,current') 'restore omitted mandatory post-child Current'
    Assert-Test ($restoreOutput.Contains('RSHELL_DISPLAY phase=before_restore arm=always flags=0 status=unavailable')) 'missing safe optional observation status'
    Assert-Test (-not $restoreOutput.Contains('optional_current_failure')) 'optional observation leaked raw exception'
    Assert-DisplayEqual $script:current $script:baseline
    Assert-Test (-not (Test-Path -LiteralPath $script:root)) 'optional observation failure prevented verified cleanup'

    foreach ($case in @('publication', 'capture', 'partial-baseline')) {
        Reset-TestCase $case
        $message = switch ($case) { 'publication' { 'publication_failure' }; 'capture' { 'capture_failure' }; default { 'baseline_write_failure' } }
        Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } $message
        Assert-Test ($script:applications.Count -eq 0 -and -not (Test-Path -LiteralPath $script:root)) 'preparation failure leaked or mutated'
        Invoke-TestRestore # Published but now absent root is a valid preparation terminal state.
    }
    Reset-TestCase 'publication'
    # Cleanup failure is supplementary and cannot replace the original failure.
    $clear = ${function:Clear-WorkspaceDisplayPreparation}
    function Clear-WorkspaceDisplayPreparation { param($Root) throw 'supplementary_cleanup_failure' }
    Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } 'publication_failure'
    Assert-Test (Test-Path -LiteralPath $script:root) 'failed preparation cleanup did not retain'
    ${function:Clear-WorkspaceDisplayPreparation} = $clear
    Reset-TestCase
    Invoke-TestRestore

    foreach ($case in @('child-failure', 'child-mismatch', 'reset-mismatch', 'final-mismatch')) {
        Reset-TestCase $case
        Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } ''
        Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'apply-started')) 'mutation failure lost obligation'
        Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'display-mode.json')) 'mutation failure lost baseline'
        Assert-Test ($script:events.Contains('Restore/Dynamic')) 'setup finally did not try recovery'
        if ($case -ne 'final-mismatch') { Assert-Test ($script:applications.Count -eq 1) 'failed arm continued experiment' }
        Reset-TestCase
        Invoke-TestRestore
    }
    foreach ($case in @('lost-ledger', 'corrupt-ledger', 'uncertain')) {
        Reset-TestCase $case
        Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } ''
        Assert-Test ($script:applications.Count -eq 1 -and -not $script:events.Contains('Restore/Dynamic')) 'unsafe recovery mutated'
        $before = $script:events.Count
        Expect-Failure { Invoke-TestRestore } ''
        Assert-Test ($script:events.Count -eq $before -and (Test-Path -LiteralPath $script:root)) 'unresolved state was mutated or cleaned'
    }
    foreach ($case in @('restore-child-failure', 'restore-mismatch', 'cleanup')) {
        Reset-TestCase
        Invoke-WorkspaceDisplaySetup $script:tempRoot
        $script:fault = $case
        $message = switch ($case) { 'restore-child-failure' { 'restore_child_failure' }; 'restore-mismatch' { 'all four fields' }; default { 'cleanup_failure' } }
        Expect-Failure { Invoke-TestRestore } $message
        Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'display-mode.json')) 'failed restore/cleanup lost baseline'
        $script:fault = 'none'
        Invoke-TestRestore
    }
    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    Expect-Failure { Invoke-WorkspaceDisplayRestore $script:root $script:tempRoot (Join-Path $script:root 'other.json') } 'ownership'
    $unknown = Join-Path $script:root 'unknown-fixture'
    # Own the extra fixture before creating it and delete only that exact file.
    try {
        Write-DisplayNewFile $unknown 'test'
        Expect-Failure { Invoke-TestRestore } 'unknown state'
        Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'display-mode.json')) 'unknown cleanup discarded ledger'
    }
    finally { [System.IO.File]::Delete($unknown) }
    Invoke-TestRestore

    # No-start preparation remnants use the same production cleanup, without Current.
    $prep = Join-Path $script:tempRoot "rshell-workspace-display-$([Guid]::NewGuid().ToString('N'))"
    $script:roots.Add($prep)
    [void](New-Item -ItemType Directory -Path $prep)
    $prepLedger = Join-Path $prep 'display-mode.json'
    Write-DisplayNewFile $prepLedger '{'
    $before = $script:events.Count
    Invoke-WorkspaceDisplayRestore $prep $script:tempRoot $prepLedger
    Assert-Test ($script:events.Count -eq $before -and -not (Test-Path -LiteralPath $prep)) 'preparation cleanup touched native seam'

    # Exercise the real fixed-child supervisor with a fixed no-native fixture.
    $fixtureRoot = Join-Path $script:tempRoot "rshell-workspace-display-$([Guid]::NewGuid().ToString('N'))"
    $script:roots.Add($fixtureRoot)
    [void](New-Item -ItemType Directory -Path $fixtureRoot)
    function New-WorkspaceDisplayChildStartInfo {
        param($Ledger, $Mode, $ChangeKind, $Target)
        $info = [System.Diagnostics.ProcessStartInfo]::new()
        $info.FileName = (Get-Command pwsh -ErrorAction Stop).Source
        foreach ($arg in @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'windows-display-timeout-fixture.ps1'))) {
            $info.ArgumentList.Add($arg)
        }
        return $info
    }
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    Expect-Failure { & $script:realChild $fixtureRoot (Join-Path $fixtureRoot 'display-mode.json') 'ApplySelected' 'Dynamic' $script:target -TimeoutMilliseconds 250 } 'timed out'
    Assert-Test ($watch.ElapsedMilliseconds -lt 10000) 'timeout/reap exceeded bounded wait'
    Assert-Test (-not (Test-Path -LiteralPath (Join-Path $fixtureRoot 'child-exit-unconfirmed'))) 'reaped child remained uncertain'
    function New-WorkspaceDisplayChildStartInfo {
        param($Ledger, $Mode, $ChangeKind, $Target)
        $info = [System.Diagnostics.ProcessStartInfo]::new()
        $info.FileName = Join-Path $fixtureRoot 'never-created.exe'
        return $info
    }
    Expect-Failure { & $script:realChild $fixtureRoot (Join-Path $fixtureRoot 'display-mode.json') 'ApplySelected' 'Dynamic' $script:target } 'did not start'
    Assert-Test (-not (Test-Path -LiteralPath (Join-Path $fixtureRoot 'child-exit-unconfirmed'))) 'confirmed no-start retained marker'
    Assert-Test (-not ('RshellDisplayConfiguration' -as [type])) 'test loaded native type'
    [Console]::WriteLine('DISPLAY_LIFETIME_NO_NATIVE_PASS arms=2 final_flags=0 timeout_reaped=1')
}
finally {
    foreach ($root in $script:roots) {
        if (Test-Path -LiteralPath $root -PathType Container) {
            if ($root -ceq $fixtureRoot -and (Test-Path -LiteralPath (Join-Path $root 'child-exit-unconfirmed'))) {
                throw 'Fixed fixture exit is uncertain; fixture state retained without cleanup.'
            }
            # Only test-owned fake uncertainty markers are removed here. Real-child
            # uncertainty would fail the assertion above; never discover or guess a PID.
            foreach ($entry in [System.IO.Directory]::EnumerateFileSystemEntries($root)) {
                Assert-Test ([System.IO.Path]::GetFileName($entry) -cin @('display-mode.json', 'apply-started', 'child-exit-unconfirmed')) 'unexpected fixture cleanup target'
                Assert-DisplayPlainPath $entry
            }
            foreach ($name in @('display-mode.json', 'apply-started', 'child-exit-unconfirmed')) {
                [System.IO.File]::Delete((Join-Path $root $name))
            }
            [System.IO.Directory]::Delete($root, $false)
        }
    }
    if (Test-Path -LiteralPath $script:tempRoot) { [System.IO.Directory]::Delete($script:tempRoot, $false) }
}
