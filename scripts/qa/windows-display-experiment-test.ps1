$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Add-Type { throw 'Add-Type is forbidden in this test.' }
$Mode = 'Restore'
$Ledger = 'fixed-import-sentinel'
. (Join-Path $PSScriptRoot 'windows-display-experiment.ps1')
if ($Mode -cne 'Restore' -or $Ledger -cne 'fixed-import-sentinel') { throw 'Display import changed caller parameters.' }

function Assert-Test {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw "Display no-native test: $Message" }
}
function Get-TestFailureText {
    param($Failure)
    $text = $Failure.Exception.Message
    if ($Failure.Exception.Data.Contains('Failures')) {
        foreach ($inner in $Failure.Exception.Data['Failures']) { $text += ';' + (Get-TestFailureText $inner) }
    }
    return $text
}
function Get-TestLeafFailures {
    param($Failure)
    if ($Failure.Exception.Data.Contains('Failures')) {
        foreach ($inner in $Failure.Exception.Data['Failures']) { Get-TestLeafFailures $inner }
    }
    else { return $Failure }
}
function Expect-Failure {
    param([scriptblock]$Action, [string]$Message = '', [switch]$PassThru, [string]$Context = '')
    $failure = $null
    try { & $Action } catch { $failure = $_ }
    Assert-Test ($null -ne $failure) "expected failure was accepted case=$Context"
    if ($Message.Length -gt 0) {
        Assert-Test ((Get-TestFailureText $failure).Contains($Message)) 'wrong failure category'
    }
    if ($PassThru) { return $failure }
}

$script:baseline = [pscustomobject]@{ Width = 1024; Height = 768; BitsPerPixel = 32; Frequency = 60 }
$script:target = [pscustomobject]@{ Width = 1920; Height = 1080; BitsPerPixel = 32; Frequency = 60 }
# Nonzero/negative virtual origin must survive capture, helper arguments and restore.
$script:areaBaseline = [pscustomobject]@{ Left = -120; Top = 30; Right = 904; Bottom = 758 }
$script:monitor = [pscustomobject]@{ Left = -120; Top = 30; Right = 1800; Bottom = 1110 }
$script:realWrite = ${function:Write-DisplayNewFile}
$script:realChild = ${function:Invoke-WorkspaceDisplayChild}
$script:realStartInfo = ${function:New-WorkspaceDisplayChildStartInfo}
$script:realHosted = ${function:Assert-WorkspaceDisplayHosted}
$script:realRemoveFile = ${function:Remove-WorkspaceDisplayFile}
$script:realRemoveDirectory = ${function:Remove-WorkspaceDisplayDirectory}
$script:realPlainPath = ${function:Assert-DisplayPlainPath}
$script:roots = [System.Collections.Generic.List[string]]::new()
$script:events = [System.Collections.Generic.List[string]]::new()
$script:applications = [System.Collections.Generic.List[string]]::new()
$script:root = ''
$script:started = '0'
$script:faults = @()
$script:modeReads = 0
$script:areaReads = 0
$script:monitorReads = 0
$script:selections = 0
$script:current = $script:baseline
$script:areaCurrent = $script:areaBaseline
$script:reparsePath = ''
$script:fixtureScenario = 'Sleep'
$fixtureRoot = ''

function Test-Fault { param([string]$Name) return $Name -cin $script:faults }
function Set-TestFault {
    param([string[]]$Faults = @())
    $script:faults = $Faults
    $script:events.Clear()
    $script:modeReads = 0
    $script:areaReads = 0
    $script:monitorReads = 0
}
function Reset-TestCase {
    param([string[]]$Faults = @())
    Set-TestFault $Faults
    $script:applications.Clear()
    $script:selections = 0
    $script:current = $script:baseline
    $script:areaCurrent = $script:areaBaseline
    $script:started = '0'
}
function Invoke-TestRestore {
    Invoke-WorkspaceDisplayRestore $script:root $script:tempRoot (Join-Path $script:root 'display-mode.json') -Started $script:started
}

# All native/hosted boundaries are replaced; these never import a native helper.
function Initialize-WorkspaceDisplayNative { throw 'Native initialization is forbidden in this test.' }
function Assert-WorkspaceDisplayHosted { param($RunnerTemp) Assert-Test ($RunnerTemp -ceq $script:tempRoot) 'unexpected hosted boundary' }
function Publish-WorkspaceDisplayRoot {
    param($Root)
    $script:root = $Root
    $script:roots.Add($Root)
    $script:events.Add('publish')
    Assert-Test (-not (Test-Path -LiteralPath (Join-Path $Root 'display-mode.json'))) 'baseline preceded publication'
    if (Test-Fault 'publication') { throw 'publication_failure' }
}
function Publish-WorkspaceDisplayStarted {
    param($Root)
    Assert-Test ($script:selections -eq 1) 'started handoff preceded target selection'
    Assert-WorkspaceDisplayReady $Root (Join-Path $Root 'display-mode.json') $script:baseline $script:areaBaseline
    $script:events.Add('started')
    if (Test-Fault 'publish-started') { throw 'started_publication_failure' }
    $script:started = [System.IO.Path]::GetFileName($Root)
    if (Test-Fault 'partial-publish-started') { throw 'partial_started_publication_failure' }
}
function Get-WorkspaceDisplayCurrent {
    $script:modeReads++
    $script:events.Add('current')
    if (Test-Fault 'capture-mode') { throw 'capture_mode_failure' }
    if ((Test-Fault 'optional-mode') -and $script:modeReads -eq 1) { throw 'optional_mode_failure' }
    if ((Test-Fault 'post-mode-read') -and $script:modeReads -eq 2) { throw 'post_mode_read_failure' }
    return $script:current
}
function Get-WorkspaceDisplayWorkArea {
    $script:areaReads++
    $script:events.Add('area')
    if (Test-Fault 'capture-area') { throw 'capture_area_failure' }
    if ((Test-Fault 'optional-area') -and $script:areaReads -eq 1) { throw 'optional_area_failure' }
    if ((Test-Fault 'post-area-read') -and $script:areaReads -eq 2) { throw 'post_area_read_failure' }
    return $script:areaCurrent
}
function Get-WorkspaceDisplayPrimaryMonitorRect {
    $script:monitorReads++
    $script:events.Add('monitor')
    Assert-Test ($script:applications.Count -eq 3) 'monitor queried before final mode child exit'
    Assert-DisplayEqual $script:current $script:target
    if (Test-Fault 'monitor-small') { return $script:areaBaseline }
    if ((Test-Fault 'monitor-change') -and $script:monitorReads -eq 2) { return $script:areaBaseline }
    return $script:monitor
}
function Select-WorkspaceDisplayTarget {
    param($Width, $Height)
    $script:selections++
    $script:events.Add('select')
    Assert-Test ($Width -eq 1920 -and $Height -eq 1080) 'minimum target weakened'
    Assert-DisplayEqual (Read-DisplayBaseline (Join-Path $script:root 'display-mode.json')) $script:baseline
    Assert-DisplayRectEqual (Read-DisplayWorkAreaBaseline (Join-Path $script:root 'workarea.json')) $script:areaBaseline
    Assert-Test (-not (Test-Path -LiteralPath (Join-Path $script:root 'apply-started'))) 'obligation preceded selection'
    if (Test-Fault 'selection') { throw 'selection_failure' }
    if (Test-Fault 'selection-corrupt-area') { [System.IO.File]::WriteAllText((Join-Path $script:root 'workarea.json'), '{}') }
    if (Test-Fault 'target-small') { return $script:baseline }
    return $script:target
}
function Write-DisplayNewFile {
    param($Path, $Text)
    $name = [System.IO.Path]::GetFileName($Path)
    if (((Test-Fault 'partial-mode') -and $name -eq 'display-mode.json') -or
        ((Test-Fault 'partial-area') -and $name -eq 'workarea.json') -or
        ((Test-Fault 'partial-started') -and $name -eq 'apply-started')) {
        & $script:realWrite $Path '{'
        throw 'partial_write_failure'
    }
    if ((Test-Fault 'write-started') -and $name -eq 'apply-started') { throw 'started_write_failure' }
    & $script:realWrite $Path $Text
    if (((Test-Fault 'corrupt-written-mode') -and $name -eq 'display-mode.json') -or
        ((Test-Fault 'corrupt-written-area') -and $name -eq 'workarea.json')) {
        [System.IO.File]::WriteAllText($Path, '{}')
    }
}
function Remove-WorkspaceDisplayFile {
    param($Path)
    $name = [System.IO.Path]::GetFileName($Path)
    if (((Test-Fault 'cleanup-first') -and $name -eq 'display-mode.json') -or
        ((Test-Fault 'cleanup-partial') -and $name -eq 'workarea.json') -or
        ((Test-Fault 'cleanup-marker') -and $name -eq 'apply-started')) { throw 'cleanup_failure' }
    if ((Test-Fault 'child-marker-removal') -and $name -eq 'child-exit-unconfirmed') { throw 'child_marker_removal_failure' }
    & $script:realRemoveFile $Path
}
function Remove-WorkspaceDisplayDirectory {
    param($Root)
    if (Test-Fault 'cleanup-directory') { throw 'directory_cleanup_failure' }
    & $script:realRemoveDirectory $Root
}
function Assert-DisplayPlainPath {
    param($Path)
    if ($Path -ceq $script:reparsePath) { throw 'Display ownership path is a reparse point.' }
    & $script:realPlainPath $Path
}
function Invoke-WorkspaceDisplayChild {
    param($Root, $Ledger, $Mode, $ChangeKind, $Target,
        $TimeoutMilliseconds = 120000, $ReapMilliseconds = 5000, $Operation = 'Display')
    Assert-WorkspaceDisplayReady $Root (Join-Path $Root 'display-mode.json') $script:baseline $script:areaBaseline $script:started
    if ($Operation -eq 'Display') {
        $script:events.Add("$Mode/$ChangeKind")
        Assert-Test ($Ledger -ceq (Join-Path $Root 'display-mode.json')) 'mode ledger changed'
        if ($Mode -eq 'ApplySelected') {
            Assert-DisplayEqual $Target $script:target
            $script:applications.Add(($Target | ConvertTo-Json -Compress))
            $script:current = $Target
            foreach ($surface in @('mode', 'area', 'started')) {
                $name = switch ($surface) { 'mode' { 'display-mode.json' }; 'area' { 'workarea.json' }; 'started' { 'apply-started' } }
                if (Test-Fault "lost-$surface") { [System.IO.File]::Delete((Join-Path $Root $name)); throw "lost_$surface" }
                if (Test-Fault "corrupt-$surface") { [System.IO.File]::WriteAllText((Join-Path $Root $name), '{'); throw "corrupt_$surface" }
            }
            if (Test-Fault 'uncertain-mode') { Write-DisplayNewFile (Join-Path $Root 'child-exit-unconfirmed') '1'; throw 'uncertain_mode' }
            if (Test-Fault 'apply-mode-failure') { throw 'apply_mode_failure' }
            if (Test-Fault 'apply-mode-mismatch') { return $script:baseline }
            if ($ChangeKind -eq 'Fullscreen') { $script:current = $script:baseline } # Observed child-exit rollback is allowed.
            if ((Test-Fault 'final-mode-mismatch') -and $script:applications.Count -eq 3) { $script:current = $script:baseline }
        }
        else {
            Assert-Test ($ChangeKind -eq 'Dynamic') 'mode restore did not use flags0'
            Assert-DisplayEqual $Target $script:baseline
            if (Test-Fault 'uncertain-restore') { Write-DisplayNewFile (Join-Path $Root 'child-exit-unconfirmed') '1'; throw 'uncertain_restore' }
            if (Test-Fault 'restore-mode-failure') { throw 'restore_mode_failure' }
            $script:current = $Target
            if (Test-Fault 'restore-mode-mismatch') { $script:current = $script:target }
        }
    }
    else {
        $script:events.Add("Workarea/$Mode")
        Assert-Test ($ChangeKind -eq 'Dynamic' -and $Ledger -ceq (Join-Path $Root 'workarea.json')) 'workarea flags/ledger changed'
        if ($Mode -eq 'ApplySelected') {
            Assert-Test ($script:applications.Count -eq 3 -and $script:monitorReads -eq 1) 'workarea preceded mode/monitor verification'
            Assert-DisplayRectEqual $Target $script:monitor
            $script:areaCurrent = $Target
            if (Test-Fault 'uncertain-area') { Write-DisplayNewFile (Join-Path $Root 'child-exit-unconfirmed') '1'; throw 'uncertain_area' }
            if (Test-Fault 'apply-area-failure') { throw 'apply_area_failure' }
            if (Test-Fault 'apply-area-child-mismatch') { return $script:areaBaseline }
            if (Test-Fault 'apply-area-parent-mismatch') { $script:areaCurrent = $script:areaBaseline }
        }
        else {
            Assert-DisplayRectEqual $Target $script:areaBaseline
            if (Test-Fault 'restore-area-failure') { throw 'restore_area_failure' }
            $script:areaCurrent = $Target
            if (Test-Fault 'restore-area-mismatch') { $script:areaCurrent = $script:monitor }
        }
    }
    return $Target
}

$tempParent = [System.IO.Path]::GetTempPath()
$approvedParent = Join-Path $tempParent 'opencode'
if (Test-Path -LiteralPath $approvedParent -PathType Container) { $tempParent = $approvedParent }
$tempParent = [System.IO.Path]::GetFullPath($tempParent)
Assert-Test (Test-Path -LiteralPath $tempParent -PathType Container) 'temporary parent missing'
$script:tempRoot = Join-Path $tempParent "rshell-display-test-$([Guid]::NewGuid().ToString('N'))"
Assert-Test (-not (Test-Path -LiteralPath $script:tempRoot)) 'fixture already exists'
$originalRunnerTemp = $env:RUNNER_TEMP
$originalConsole = [Console]::Out
$originalErrorConsole = [Console]::Error
$output = [System.IO.StringWriter]::new()
$errorOutput = [System.IO.StringWriter]::new()
try {
    [void](New-Item -ItemType Directory -Path $script:tempRoot)
    $env:RUNNER_TEMP = $script:tempRoot
    [Console]::SetOut($output)
    [Console]::SetError($errorOutput)
    Assert-Test (-not ('RshellDisplayConfiguration' -as [type])) 'import loaded native type'
    # The real guard is exercised only in a guaranteed rejected environment.
    $actions = $env:GITHUB_ACTIONS
    try {
        $env:GITHUB_ACTIONS = 'false'
        Expect-Failure { & $script:realHosted $script:tempRoot } 'hosted Windows'
    }
    finally { $env:GITHUB_ACTIONS = $actions }

    foreach ($value in @($null, 'rectangle', @(), [pscustomobject]@{}, [pscustomobject]@{ Left = '0'; Top = 0; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = 0; Top = 0; Right = 1 },
        [pscustomobject]@{ left = 0; Top = 0; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = @(0); Top = 0; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = $null; Top = 0; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = $false; Top = 0; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = [uint32]0; Top = 0; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = 0; Top = 0; Right = 1; Bottom = 1; Extra = 1 },
        [pscustomobject]@{ Left = 0; Top = 0; Right = 0; Bottom = 1 },
        [pscustomobject]@{ Left = 0; Top = 1; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = 0; Top = 0; Right = 2147483648L; Bottom = 1 },
        [pscustomobject]@{ Left = -2147483649L; Top = 0; Right = 1; Bottom = 1 },
        [pscustomobject]@{ Left = 0.0; Top = 0; Right = 1; Bottom = 1 })) {
        Expect-Failure { ConvertTo-DisplayRect $value } 'RECT is invalid'
    }
    Assert-DisplayRectEqual (ConvertTo-DisplayRect $script:monitor) $script:monitor
    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    $expected = 'publish,current,area,select,started,ApplySelected/Fullscreen,current,Restore/Dynamic,current,ApplySelected/Dynamic,current,ApplySelected/Dynamic,current,monitor,Workarea/ApplySelected,current,monitor,area'
    Assert-Test (($script:events -join ',') -ceq $expected) 'production arm/reset/final/workarea ordering changed'
    Assert-Test ($script:selections -eq 1 -and $script:applications.Count -eq 3) 'selection was repeated'
    Assert-Test (@($script:applications | Select-Object -Unique).Count -eq 1) 'experimental targets differed'
    foreach ($name in @('display-mode.json', 'workarea.json')) {
        $path = Join-Path $script:root $name
        $saved = [System.IO.File]::ReadAllText($path)
        Expect-Failure { Write-DisplayNewFile $path '{}' }
        Assert-Test ([System.IO.File]::ReadAllText($path) -ceq $saved) 'original baseline overwritten'
    }
    Set-TestFault
    Invoke-TestRestore
    Assert-Test (($script:events -join ',') -ceq 'current,area,Restore/Dynamic,current,Workarea/Restore,area') 'independent restore/read order changed'
    Assert-Test (-not (Test-Path -LiteralPath $script:root)) 'verified restore did not clean'
    # A started handoff must NEVER turn a missing root into no-op success.
    Expect-Failure { Invoke-TestRestore } 'Started display root is missing'
    Expect-Failure { Invoke-WorkspaceDisplayRestore '' $script:tempRoot '' -Started $script:started } 'root handoff'

    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    Set-TestFault @('optional-mode', 'optional-area')
    Invoke-TestRestore
    Assert-Test (($script:events -join ',') -ceq 'current,area,Restore/Dynamic,current,Workarea/Restore,area') 'optional diagnostics suppressed required restoration'
    Assert-Test (-not (Test-Path -LiteralPath $script:root)) 'optional read failure prevented cleanup'

    foreach ($case in @('publication', 'capture-mode', 'capture-area', 'partial-mode', 'partial-area', 'corrupt-written-mode',
        'corrupt-written-area', 'selection', 'selection-corrupt-area', 'target-small', 'write-started', 'partial-started', 'publish-started')) {
        Reset-TestCase @($case)
        Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } 'Workspace display setup failed' -Context $case
        Assert-Test ($script:applications.Count -eq 0 -and -not $script:events.Contains('Workarea/ApplySelected')) 'preparation mutated'
        Assert-Test (-not (Test-Path -LiteralPath $script:root)) 'known preparation failure leaked'
        Invoke-TestRestore # Explicit no-start handoff only, not marker-absence inference.
    }
    Reset-TestCase @('partial-publish-started')
    Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } 'partial_started_publication_failure'
    Assert-Test ($script:applications.Count -eq 0 -and -not (Test-Path -LiteralPath $script:root)) 'partial started publication mutated or leaked preparation'
    Expect-Failure { Invoke-TestRestore } 'Started display root is missing'
    Reset-TestCase @('publication', 'cleanup-directory')
    $failure = Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } 'publication_failure' -PassThru
    Assert-Test ((Get-TestFailureText $failure).Contains('directory_cleanup_failure')) 'preparation cleanup replaced/lost primary error'
    Assert-Test (Test-Path -LiteralPath $script:root) 'failed preparation cleanup did not retain state'
    Set-TestFault
    Invoke-TestRestore

    foreach ($case in @('apply-mode-failure', 'apply-mode-mismatch', 'restore-mode-mismatch', 'final-mode-mismatch',
        'monitor-small', 'monitor-change', 'apply-area-failure', 'apply-area-child-mismatch', 'apply-area-parent-mismatch')) {
        Reset-TestCase @($case)
        Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot }
        Assert-Test ($script:events.Contains('Restore/Dynamic') -and $script:events.Contains('Workarea/Restore')) 'setup failure skipped independent dual recovery'
        Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'apply-started')) 'setup recovery lost published obligation'
        Assert-DisplayEqual (Read-DisplayBaseline (Join-Path $script:root 'display-mode.json')) $script:baseline
        Assert-DisplayRectEqual (Read-DisplayWorkAreaBaseline (Join-Path $script:root 'workarea.json')) $script:areaBaseline
        Set-TestFault
        Invoke-TestRestore
    }
    Reset-TestCase @('apply-area-failure', 'restore-mode-failure', 'restore-area-failure')
    # This combination fails at between-mode reset before workarea Apply. Both
    # independent cleanup errors still belong to the original setup failure.
    $failure = Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot } 'restore_mode_failure' -PassThru
    Assert-Test ((Get-TestFailureText $failure).Contains('restore_area_failure')) 'setup supplementary error lost'
    Set-TestFault
    Invoke-TestRestore

    foreach ($case in @('lost-mode', 'corrupt-mode', 'lost-area', 'corrupt-area', 'lost-started', 'corrupt-started', 'uncertain-mode', 'uncertain-area')) {
        Reset-TestCase @($case)
        Expect-Failure { Invoke-WorkspaceDisplaySetup $script:tempRoot }
        if ($case -ne 'uncertain-area') { Assert-Test (-not $script:events.Contains('Restore/Dynamic')) 'unsafe setup recovery spawned child' }
        Assert-Test (-not $script:events.Contains('Workarea/Restore')) 'unsafe setup recovery SET workarea'
        $before = $script:events.Count
        Expect-Failure { Invoke-TestRestore }
        Assert-Test ($script:events.Count -eq $before -and (Test-Path -LiteralPath $script:root)) 'untrusted restore touched native or discarded state'
    }

    foreach ($case in @('restore-mode-failure', 'restore-mode-mismatch', 'post-mode-read', 'restore-area-failure', 'restore-area-mismatch', 'post-area-read')) {
        Reset-TestCase
        Invoke-WorkspaceDisplaySetup $script:tempRoot
        Set-TestFault @($case)
        Expect-Failure { Invoke-TestRestore }
        Assert-Test (($script:events -join ',') -ceq 'current,area,Restore/Dynamic,current,Workarea/Restore,area') 'confirmed failure suppressed independent operation/post-read'
        Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'apply-started')) 'unverified restore retired obligation'
        Set-TestFault
        Invoke-TestRestore # Idempotent safe retry with immutable originals.
    }
    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    Set-TestFault @('restore-mode-failure', 'restore-area-failure', 'post-mode-read', 'post-area-read')
    $failure = Expect-Failure { Invoke-TestRestore } 'restore_mode_failure' -PassThru
    foreach ($message in @('restore_area_failure', 'post_mode_read_failure', 'post_area_read_failure')) {
        Assert-Test ((Get-TestFailureText $failure).Contains($message)) 'dual failure/post-read error lost'
    }
    Assert-Test (@(Get-TestLeafFailures $failure).Count -eq 4) 'independent original errors were not retained'
    Set-TestFault
    Invoke-TestRestore
    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    Set-TestFault @('uncertain-restore')
    Expect-Failure { Invoke-TestRestore } 'restoration incomplete'
    Assert-Test (($script:events -join ',') -ceq 'current,area,Restore/Dynamic') 'unconfirmed exit allowed competing query/SET'
    $before = $script:events.Count
    Expect-Failure { Invoke-TestRestore }
    Assert-Test ($script:events.Count -eq $before) 'uncertain retry touched native'

    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    $before = $script:events.Count
    foreach ($handoff in @('', '0', 'rshell-workspace-display-00000000000000000000000000000000')) {
        Expect-Failure { Invoke-WorkspaceDisplayRestore $script:root $script:tempRoot (Join-Path $script:root 'display-mode.json') -Started $handoff } 'handoff'
    }
    Expect-Failure { Invoke-WorkspaceDisplayRestore $script:root $script:tempRoot (Join-Path $script:root 'other.json') -Started $script:started } 'ownership'
    Expect-Failure { Invoke-WorkspaceDisplayRestore (Join-Path $script:tempRoot 'not-owned') $script:tempRoot '' -Started $script:started } 'run-owned'
    $unknown = Join-Path $script:root 'unknown-fixture'
    try {
        Write-DisplayNewFile $unknown 'test'
        Expect-Failure { Invoke-TestRestore } 'unknown state'
    }
    finally { [System.IO.File]::Delete($unknown) }
    foreach ($name in @('', 'display-mode.json', 'workarea.json', 'apply-started')) {
        $script:reparsePath = if ($name -eq '') { $script:root } else { Join-Path $script:root $name }
        Expect-Failure { Invoke-TestRestore } 'reparse point'
    }
    $script:reparsePath = ''
    Assert-Test ($script:events.Count -eq $before) 'invalid ownership/handoff reached native'
    Invoke-TestRestore

    Reset-TestCase
    Invoke-WorkspaceDisplaySetup $script:tempRoot
    Set-TestFault @('cleanup-first')
    Expect-Failure { Invoke-TestRestore } 'cleanup_failure'
    Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'display-mode.json')) 'early cleanup failure lost baseline'
    Set-TestFault
    Invoke-TestRestore
    foreach ($case in @('cleanup-partial', 'cleanup-marker', 'cleanup-directory')) {
        Reset-TestCase
        Invoke-WorkspaceDisplaySetup $script:tempRoot
        Set-TestFault @($case)
        $logStart = $output.GetStringBuilder().Length
        Expect-Failure { Invoke-TestRestore } 'cleanup_failure'
        Assert-Test (Test-Path -LiteralPath (Join-Path $script:root 'apply-started')) 'partial deletion lost started proof'
        Assert-Test (-not (Test-Path -LiteralPath (Join-Path $script:root 'display-mode.json'))) 'fixture failed to exercise partial deletion'
        Assert-Test (-not $output.ToString().Substring($logStart).Contains('cleanup=complete')) 'partial deletion reported complete'
        Set-TestFault
        Expect-Failure { Invoke-TestRestore }
        Assert-Test ($script:events.Count -eq 0 -and (Test-Path -LiteralPath $script:root)) 'partial deletion was silently accepted or guessed a baseline'
    }

    # Explicit preparation handoff can clean malformed partial files without GET.
    Reset-TestCase
    $script:root = Join-Path $script:tempRoot "rshell-workspace-display-$([Guid]::NewGuid().ToString('N'))"
    $script:roots.Add($script:root)
    [void](New-Item -ItemType Directory -Path $script:root)
    Write-DisplayNewFile (Join-Path $script:root 'display-mode.json') '{'
    Write-DisplayNewFile (Join-Path $script:root 'workarea.json') '{'
    Invoke-TestRestore
    Assert-Test ($script:events.Count -eq 0 -and -not (Test-Path -LiteralPath $script:root)) 'preparation cleanup touched native'
    Invoke-WorkspaceDisplayRestore '' $script:tempRoot '' -Started '' # No setup handoff at all.

    # Inspect production argument construction without starting either native helper.
    $info = & $script:realStartInfo 'original-ledger' 'ApplySelected' 'Dynamic' $script:monitor 'Workarea'
    Assert-Test (($info.ArgumentList -join ',').EndsWith('-Mode,ApplySelected,-Ledger,original-ledger,-Left,-120,-Top,30,-Right,1800,-Bottom,1110')) 'workarea fixed-child interface changed'
    $info = & $script:realStartInfo 'original-ledger' 'Restore' 'Dynamic' $script:baseline
    Assert-Test (($info.ArgumentList -join ',').Contains('-ChangeKind,Dynamic,-Width,1024,-Height,768,-BitsPerPixel,32,-Frequency,60')) 'mode flags0 arguments changed'

    # Real fixed sleeping fixture: exercise timeout + kill + bounded confirmed reap.
    $fixtureRoot = Join-Path $script:tempRoot "rshell-workspace-display-$([Guid]::NewGuid().ToString('N'))"
    $script:roots.Add($fixtureRoot)
    [void](New-Item -ItemType Directory -Path $fixtureRoot)
    $fixtureLedger = Join-Path $fixtureRoot 'display-mode.json'
    $fixtureAreaLedger = Join-Path $fixtureRoot 'workarea.json'
    Write-DisplayNewFile $fixtureLedger ($script:baseline | ConvertTo-Json -Compress)
    Write-DisplayNewFile $fixtureAreaLedger ($script:areaBaseline | ConvertTo-Json -Compress)
    Write-DisplayNewFile (Join-Path $fixtureRoot 'apply-started') ([System.IO.Path]::GetFileName($fixtureRoot))
    function New-WorkspaceDisplayChildStartInfo {
        param($Ledger, $Mode, $ChangeKind, $Target, $Operation = 'Display')
        $info = [System.Diagnostics.ProcessStartInfo]::new()
        $info.FileName = (Get-Command pwsh -ErrorAction Stop).Source
        foreach ($arg in @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'windows-display-timeout-fixture.ps1'), '-Scenario', $script:fixtureScenario)) {
            $info.ArgumentList.Add($arg)
        }
        return $info
    }
    $fixedFixtureStartInfo = ${function:New-WorkspaceDisplayChildStartInfo}
    foreach ($operation in @('Display', 'Workarea')) {
        $ledger = if ($operation -eq 'Display') { $fixtureLedger } else { $fixtureAreaLedger }
        $fixtureTarget = if ($operation -eq 'Display') { $script:target } else { $script:monitor }
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        Expect-Failure { & $script:realChild $fixtureRoot $ledger 'ApplySelected' 'Dynamic' $fixtureTarget -Operation $operation -TimeoutMilliseconds 250 } 'timed out'
        Assert-Test ($watch.ElapsedMilliseconds -lt 10000) 'fixed timeout/reap exceeded bound'
        Assert-Test (-not (Test-Path -LiteralPath (Join-Path $fixtureRoot 'child-exit-unconfirmed'))) 'reaped fixture left uncertainty'
    }
    foreach ($operation in @('Display', 'Workarea')) {
        foreach ($mode in @('ApplySelected', 'Restore')) {
            $ledger = if ($operation -eq 'Display') { $fixtureLedger } else { $fixtureAreaLedger }
            $script:fixtureScenario = if ($mode -eq 'Restore') { "${operation}Restore" } else { $operation }
            $fixtureTarget = if ($operation -eq 'Display') { if ($mode -eq 'Restore') { $script:baseline } else { $script:target } }
                else { if ($mode -eq 'Restore') { $script:areaBaseline } else { $script:monitor } }
            $actual = & $script:realChild $fixtureRoot $ledger $mode 'Dynamic' $fixtureTarget -Operation $operation -TimeoutMilliseconds 5000
            if ($operation -eq 'Display') { Assert-DisplayEqual $actual $fixtureTarget } else { Assert-DisplayRectEqual $actual $fixtureTarget }
        }
    }
    foreach ($scenario in @('Invalid', 'Overflow', 'ExtraLine', 'Stderr', 'Nonzero')) {
        $script:fixtureScenario = $scenario
        Expect-Failure { & $script:realChild $fixtureRoot $fixtureAreaLedger 'ApplySelected' 'Dynamic' $script:monitor -Operation Workarea -TimeoutMilliseconds 5000 }
        Assert-Test (-not (Test-Path -LiteralPath (Join-Path $fixtureRoot 'child-exit-unconfirmed'))) 'confirmed malformed child exit retained uncertainty'
    }
    function New-WorkspaceDisplayChildStartInfo {
        param($Ledger, $Mode, $ChangeKind, $Target, $Operation = 'Display')
        $info = [System.Diagnostics.ProcessStartInfo]::new()
        $info.FileName = Join-Path $fixtureRoot 'never-created.exe'
        return $info
    }
    Expect-Failure { & $script:realChild $fixtureRoot $fixtureLedger 'ApplySelected' 'Dynamic' $script:target } 'did not start'
    Assert-Test (-not (Test-Path -LiteralPath (Join-Path $fixtureRoot 'child-exit-unconfirmed'))) 'confirmed no-start retained marker'

    # The real fixed fixture exits 7; failure to remove its marker must not replace
    # that original supervisor error. Reuse the coordinator's narrow delete seam.
    ${function:New-WorkspaceDisplayChildStartInfo} = $fixedFixtureStartInfo
    $script:fixtureScenario = 'Nonzero'
    Set-TestFault @('child-marker-removal')
    $fixtureMarker = Join-Path $fixtureRoot 'child-exit-unconfirmed'
    $fixtureExitConfirmed = $false
    $logStart = $output.GetStringBuilder().Length
    try {
        $failure = Expect-Failure { & $script:realChild $fixtureRoot $fixtureLedger 'ApplySelected' 'Dynamic' $script:target -TimeoutMilliseconds 5000 } -PassThru
        Assert-Test ($output.ToString().Substring($logStart).Contains('RSHELL_DISPLAY child_exit=7')) 'dual-error fixture exit was not confirmed'
        $fixtureExitConfirmed = $true
        Assert-Test ($failure.Exception.Message -ceq 'Display child failed; exit evidence retained.') 'dual-error supervisor message was not fixed safe'
        $errors = @($failure.Exception.Data['Failures'])
        Assert-Test ($errors.Count -eq 2) 'supervisor lost original failure or marker-removal error'
        Assert-Test ($errors[0].Exception.Message -ceq 'Display child failed.') 'marker cleanup replaced original supervisor failure'
        Assert-Test ($errors[1].Exception.Message -ceq 'child_marker_removal_failure') 'supplementary marker-removal error was not preserved'
        Assert-Test ([System.IO.File]::ReadAllText($fixtureMarker) -ceq '1') 'marker-removal failure discarded exit evidence'
    }
    finally {
        Set-TestFault
        # Only this exact fixture's confirmed exit permits test-owned retirement.
        if ($fixtureExitConfirmed) { [System.IO.File]::Delete($fixtureMarker) }
    }

    # Real non-Win32 Process.Start exception, without creating ANY process. The
    # production supervisor conservatively retains its uncertainty obligation.
    $script:uncertainStartConstructions = 0
    $script:blockedWorkAreaSets = 0
    function New-WorkspaceDisplayChildStartInfo {
        param($Ledger, $Mode, $ChangeKind, $Target, $Operation = 'Display')
        $script:uncertainStartConstructions++
        return [System.Diagnostics.ProcessStartInfo]::new() # Deliberately no FileName.
    }
    function Set-WorkspaceWorkArea {
        param($Rect)
        $script:blockedWorkAreaSets++
        throw 'Workarea SET is forbidden during uncertain-start recovery.'
    }
    $savedState = @{}
    foreach ($name in @('display-mode.json', 'workarea.json', 'apply-started')) {
        $savedState[$name] = [System.IO.File]::ReadAllText((Join-Path $fixtureRoot $name))
    }
    $noProcessCreated = $false
    $mockChild = ${function:Invoke-WorkspaceDisplayChild}
    $logStart = $output.GetStringBuilder().Length
    $errorLogStart = $errorOutput.GetStringBuilder().Length
    try {
        $failure = Expect-Failure { & $script:realChild $fixtureRoot $fixtureLedger 'ApplySelected' 'Dynamic' $script:target } -PassThru
        $startErrors = @(Get-TestLeafFailures $failure)
        Assert-Test ($startErrors.Count -eq 1 -and $startErrors[0].Exception.GetBaseException() -is [System.InvalidOperationException]) 'fixture did not exercise the real non-Win32 start exception'
        $noProcessCreated = $true # Missing FileName fails before OS process creation.
        Assert-Test ($failure.Exception.Message -ceq 'Display child start failed; exit unconfirmed; state retained.') 'uncertain-start error was not fixed safe'
        Assert-Test ([System.IO.File]::ReadAllText($fixtureMarker) -ceq '1') 'real uncertain-start branch cleared its marker'

        # Restore uses the real supervisor symbol as well as the real coordinator;
        # query/SET tripwires remain mocked and must receive zero calls.
        ${function:Invoke-WorkspaceDisplayChild} = $script:realChild
        $failure = Expect-Failure {
            Invoke-WorkspaceDisplayRestore $fixtureRoot $script:tempRoot $fixtureLedger -Started ([System.IO.Path]::GetFileName($fixtureRoot))
        } 'Display child exit is unconfirmed; state retained.' -PassThru
        Assert-Test ($failure.Exception.Message -ceq 'Workspace display restoration failed; state retained.') 'blocked restore error was not fixed safe'
        Assert-Test ($script:events.Count -eq 0 -and $script:modeReads -eq 0 -and $script:areaReads -eq 0 -and $script:monitorReads -eq 0) 'uncertain-start restore queried parent state'
        Assert-Test ($script:uncertainStartConstructions -eq 1 -and $script:blockedWorkAreaSets -eq 0) 'uncertain-start restore constructed another child or SET workarea'
        Assert-Test ([System.IO.File]::ReadAllText($fixtureMarker) -ceq '1') 'blocked restore discarded uncertainty evidence'
        foreach ($name in @('display-mode.json', 'workarea.json', 'apply-started')) {
            Assert-Test ([System.IO.File]::ReadAllText((Join-Path $fixtureRoot $name)) -ceq $savedState[$name]) 'blocked restore changed or cleared recovery files'
        }
        Assert-Test ($output.GetStringBuilder().Length -eq $logStart -and $errorOutput.GetStringBuilder().Length -eq $errorLogStart) 'uncertain-start path printed raw error payload or completion'
    }
    finally {
        ${function:Invoke-WorkspaceDisplayChild} = $mockChild
        # This deliberately invalid StartInfo is proven to have created no child;
        # only test fixture teardown may clear its conservative production marker.
        if ($noProcessCreated) { [System.IO.File]::Delete($fixtureMarker) }
    }

    # Exercise the production workarea command body with only public function
    # replacements. The helper stays import-only; no native setter is reachable.
    $script:childSets = 0
    $script:childRect = $null
    function Get-WorkspacePrimaryMonitorRect { return $script:monitor }
    function Set-WorkspaceWorkArea {
        param($Rect)
        $script:childSets++
        $script:childRect = $Rect
        if (Test-Fault 'command-set-failure') { throw 'command_set_failure' }
        if (Test-Fault 'command-nonconverge') { return $script:areaBaseline }
        return $Rect
    }
    Write-DisplayNewFile (Join-Path $fixtureRoot 'child-exit-unconfirmed') '1'
    try {
        Invoke-WorkspaceWorkAreaChildOperation 'ApplySelected' $fixtureAreaLedger $script:monitor
        Assert-DisplayRectEqual $script:childRect $script:monitor
        Invoke-WorkspaceWorkAreaChildOperation 'Restore' $fixtureAreaLedger $script:monitor
        Assert-DisplayRectEqual $script:childRect $script:areaBaseline
        $before = $script:childSets
        Expect-Failure { Invoke-WorkspaceWorkAreaChildOperation 'ApplySelected' $fixtureAreaLedger $script:areaBaseline } 'all four fields'
        Expect-Failure { Invoke-WorkspaceWorkAreaChildOperation 'Restore' $fixtureLedger $script:monitor } 'ownership'
        Assert-Test ($script:childSets -eq $before) 'invalid workarea command reached setter'
        Set-TestFault @('command-set-failure')
        Expect-Failure { Invoke-WorkspaceWorkAreaChildOperation 'ApplySelected' $fixtureAreaLedger $script:monitor } 'command_set_failure'
        Set-TestFault @('command-nonconverge')
        Expect-Failure { Invoke-WorkspaceWorkAreaChildOperation 'ApplySelected' $fixtureAreaLedger $script:monitor } 'all four fields'
        Set-TestFault
    }
    finally { [System.IO.File]::Delete((Join-Path $fixtureRoot 'child-exit-unconfirmed')) }
    $before = $script:childSets
    Expect-Failure { Invoke-WorkspaceWorkAreaChildOperation 'Restore' $fixtureAreaLedger $script:monitor }
    Assert-Test ($script:childSets -eq $before) 'unowned workarea command reached setter'
    foreach ($typeName in @('RshellDisplayConfiguration', 'RshellWorkAreaConfiguration', 'RshellWorkAreaRect')) {
        Assert-Test (-not ($typeName -as [type])) 'suite loaded native type'
    }
    # Every successful console line is from a finite enum/numeric protocol; no
    # root, ledger path, exception, arbitrary payload or child stderr is printed.
    foreach ($line in ($output.ToString() -split '\r?\n' | Where-Object { $_.Length -gt 0 })) {
        Assert-Test ($line -cmatch '^RSHELL_DISPLAY (phase=(baseline|target|child_applied|parent_after_exit|before_restore|restore_after_exit) arm=(setup|fullscreen|dynamic|between|final|recovery|always) flags=(0|4) (width=[0-9]+ height=[0-9]+ bpp=[0-9]+ frequency=[0-9]+|status=unavailable)|discrimination=insufficient|cleanup=complete|child_exit=-?[0-9]+|child_reaped_exit=-?[0-9]+)$' -or
            $line -cmatch '^RSHELL_WORKAREA phase=(baseline|target|child_applied|parent_after_exit|before_restore|restore_after_exit) arm=(setup|final|recovery|always) flags=0 (left=-?[0-9]+ top=-?[0-9]+ right=-?[0-9]+ bottom=-?[0-9]+|status=unavailable)$' -or
            $line -cmatch '^RSHELL_WORKAREA_(CHILD|RESTORED) left=-?[0-9]+ top=-?[0-9]+ right=-?[0-9]+ bottom=-?[0-9]+ flags=0$') 'unsafe/unbounded production console line'
    }
    foreach ($line in ($errorOutput.ToString() -split '\r?\n' | Where-Object { $_.Length -gt 0 })) {
        Assert-Test ($line -cin @('RSHELL_DISPLAY setup_cleanup_failed state=retained', 'RSHELL_DISPLAY obligation_retirement_failed state=retained')) 'raw cleanup error output'
    }
}
finally {
    [Console]::SetOut($originalConsole)
    [Console]::SetError($originalErrorConsole)
    $output.Dispose()
    $errorOutput.Dispose()
    $env:RUNNER_TEMP = $originalRunnerTemp
    foreach ($root in $script:roots) {
        if (Test-Path -LiteralPath $root -PathType Container) {
            if ($root -ceq $fixtureRoot -and (Test-Path -LiteralPath (Join-Path $root 'child-exit-unconfirmed'))) {
                throw 'Fixed fixture exit is uncertain; fixture state retained without cleanup.'
            }
            # Only test-owned fake uncertainty is cleared. No PID discovery,
            # recursive deletion, unknown entries or actual native recovery.
            foreach ($entry in [System.IO.Directory]::EnumerateFileSystemEntries($root)) {
                Assert-Test ([System.IO.Path]::GetFileName($entry) -cin @('display-mode.json', 'workarea.json', 'apply-started', 'child-exit-unconfirmed')) 'unexpected fixture cleanup target'
                & $script:realPlainPath $entry
            }
            foreach ($name in @('display-mode.json', 'workarea.json', 'apply-started', 'child-exit-unconfirmed')) { [System.IO.File]::Delete((Join-Path $root $name)) }
            [System.IO.Directory]::Delete($root, $false)
        }
    }
    if (Test-Path -LiteralPath $script:tempRoot) { [System.IO.Directory]::Delete($script:tempRoot, $false) }
}
[Console]::WriteLine('DISPLAY_LIFETIME_NO_NATIVE_PASS arms=2 final_flags=0 workarea_flags=0 timeout_reaped=2')
[Console]::WriteLine('DISPLAY_CHILD_FAILURE_NO_NATIVE_PASS uncertain_start=1 blocked_restore=1 retained_errors=2')
