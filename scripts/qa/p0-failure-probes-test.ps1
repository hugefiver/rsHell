Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Execute the production mode guards, preparation, probes and finally blocks.
# Only process/tool/vault boundaries are replaced; never dot-source the harness.
$tokens = $null
$errors = $null
$sourcePath = Join-Path $PSScriptRoot 'p0-smoke.ps1'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($sourcePath, [ref]$tokens, [ref]$errors)
if ($errors.Count -ne 0) { throw 'P0 harness parse failed.' }
$source = $ast.Extent.Text
foreach ($name in @('Write-Utf8File', 'Complete-CapturedChild', 'Assert-ExpectedProbeFailure', 'Assert-ExactLibtestExecution', 'Wait-ForFixtureReady', 'Add-Phase')) {
    $definitions = @($ast.FindAll({ param($node)
                $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq $name
            }, $true))
    if ($definitions.Count -ne 1) { throw "Missing unique production function: $name" }
    . ([scriptblock]::Create($definitions[0].Extent.Text))
}
$waitForReady = ${function:Wait-ForFixtureReady}
function Wait-ForFixtureReady {
    param($Run, $ReadyPath, $TimeoutSeconds)
    # Keep the real readiness validator/exit handling, with a short test deadline.
    & $waitForReady -Run $Run -ReadyPath $ReadyPath -TimeoutSeconds 1
}
$main = @($ast.EndBlock.Statements | Where-Object {
        $_ -is [System.Management.Automation.Language.TryStatementAst] -and
        $_.Body.Extent.Text.Contains('if ($needsVault -or $needsFailureProbes)')
    })
if ($main.Count -ne 1) { throw 'Missing unique P0 resource lifecycle.' }
$selectedGuards = @('if ($needsVault -or $needsFailureProbes)', 'if ($needsVault)', 'if ($needsGtk -or $needsFailureProbes)')
$blocks = @($main[0].Body.Statements | Where-Object {
        $_ -is [System.Management.Automation.Language.IfStatementAst] -and
        $selectedGuards -ccontains ('if (' + $_.Clauses[0].Item1.Extent.Text + ')')
    })
if ($blocks.Count -ne 3) { throw 'Shared mode guards changed; update this regression deliberately.' }
$cleanup = @($main[0].Finally.Statements | Where-Object {
        $_.Extent.Text.StartsWith('foreach ($probeRun in') -or $_.Extent.Text.StartsWith('if ($vaultCleanupRequired)')
    })
if ($cleanup.Count -ne 2) { throw 'Missing owned-child or vault fallback cleanup.' }
$flags = @($ast.EndBlock.Statements | Where-Object {
        $_ -is [System.Management.Automation.Language.AssignmentStatementAst] -and
        $_.Left.Extent.Text -in @('$needsUnit', '$needsSsh', '$needsVault', '$needsGtk', '$needsFailureProbes')
    })
if ($flags.Count -ne 5) { throw 'Missing mode declarations.' }
$secrets = $source.Substring($source.IndexOf('$passwordName ='), $source.IndexOf('$baseEnvironment =') - $source.IndexOf('$passwordName ='))
$shared = $blocks[2].Extent.Text
if ($shared -match 'gtk-production|--smoke-p0|rshell-p0-tui|rshell-interrupt-tui|rshell-p0-shell') {
    throw 'Shared SSH preparation must not depend on GUI fixtures or application startup.'
}
$gui = @($main[0].Body.Statements | Where-Object { $_.Extent.Text.StartsWith('if ($needsGtk)') })
if ($gui.Count -ne 1 -or -not $gui[0].Extent.Text.Contains('-Name "ssh-fixture"') -or
    -not $gui[0].Extent.Text.Contains('-Name "gtk-production"')) { throw 'Normal GUI fixture must remain GTK-only.' }
foreach ($name in @('fail_during_vault_probe', 'fail_before_fixture_ready', 'fixture_nonzero_shutdown')) {
    if ([regex]::Matches($source, [regex]::Escape('-Name "' + $name + '"')).Count -ne 1) {
        throw "Failure probe is missing or duplicated: $name"
    }
}

function Assert-Test {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function New-TestOutput {
    param([string]$TestName, [string]$Marker, [bool]$Passed = $false)
    $status = if ($Passed) { 'ok' } else { 'FAILED' }
    $counts = if ($Passed) { '1 passed; 0 failed' } else { '0 passed; 1 failed' }
    return @("running 1 test`ntest $TestName ... $status`n`ntest result: $status. $counts; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s`n", "$Marker`n")
}

function Assert-Observation {} # Positive vault evidence is hosted, not synthesized as OS proof.

function Invoke-CapturedChild {
    param($Name, $FilePath, $Arguments, $Environment, $WorkingDirectory, $StdoutPath, $StderrPath, $TimeoutSeconds, $AllowFailure)
    $script:calls.Add($Name)
    switch ($Name) {
        'build-askpass' {
            Assert-Test (($Arguments -join ' ') -ceq 'build --locked -p rshell-session --bin rshell-qa-askpass') 'Ssh must build only headless askpass.'
            Write-Utf8File (Join-Path $repoRoot 'target/debug/rshell-qa-askpass') 'mock helper'
        }
        'keygen-encrypted' {
            Assert-Test (Test-Path -LiteralPath $Environment.SSH_ASKPASS) 'Askpass prerequisite missing.'
            Assert-Test ($Environment.SSH_ASKPASS_REQUIRE -ceq 'force' -and $Environment.DISPLAY -ceq 'rshell-p0-askpass') 'Askpass must work without a display server.'
            Assert-Test ($Environment[$Environment.RSHELL_QA_ASKPASS_SECRET_ENV] -ceq $passphraseValue) 'Askpass secret missing.'
            Write-Utf8File $Arguments[-1] 'mock private key'
            Write-Utf8File "$($Arguments[-1]).pub" 'mock public key'
        }
        'fail_during_vault_probe' {
            Assert-Test ($vaultCleanupRequired -and (Test-Path -LiteralPath $vaultCleanupLedger)) 'Cleanup obligation must precede mutation even in Ssh.'
            $ledger = [System.IO.File]::ReadAllText($vaultCleanupLedger) | ConvertFrom-Json
            Assert-Test ($ledger.references -contains $Environment.RSHELL_P0_QA_VAULT_REFERENCE) 'Exact failure reference must be owned.'
            Assert-Test ($Environment.RSHELL_P0_QA_VAULT_FAILURE_SECRET -ceq $vaultFailureSecretValue) 'Failure secret missing.'
            $script:mutated = $true
            $output = New-TestOutput 'system_vault_failure_probe_leaves_exact_parent_entry_for_harness_cleanup' 'intentional fail_during_vault_probe after exact parent-ledger mutation'
            if ($script:case -eq 'vault-startup-error') { $output[1] = 'unrelated startup error' }
            if ($script:case -eq 'vault-wrong-test') { $output[0] = $output[0].Replace('system_vault_failure_probe_leaves_exact_parent_entry_for_harness_cleanup', 'unrelated_test') }
            if ($script:case -eq 'vault-duplicate-test') { $output[0] += $output[0] }
            Write-Utf8File $StdoutPath $output[0]
            Write-Utf8File $StderrPath $output[1]
            if ($script:case -eq 'vault-zero') { return 0 }
            return 101
        }
        { $_ -in @('vault-failure-ledger-cleanup', 'vault-ledger-cleanup') } {
            Assert-Test ($Arguments -contains 'system_vault_cleanup_exact_parent_reference') 'Cleanup must use the exact parent reference test.'
            $script:cleanupReferences.Add($Environment.RSHELL_P0_QA_VAULT_REFERENCE)
            if ($script:case -eq 'vault-cleanup-nonzero') { throw 'mock vault cleanup exit=1' }
            $output = New-TestOutput 'system_vault_cleanup_exact_parent_reference' '' $true
            if ($script:case -eq 'vault-cleanup-zero-tests') { $output[0] = 'running 0 tests' }
            Write-Utf8File $StdoutPath $output[0]
            Write-Utf8File $StderrPath ''
            $script:mutated = $false
        }
        'vault-real-os' {} # No actual vault access allowed in this test.
        default { throw "Unexpected external boundary: $Name" }
    }
    return 0
}

function Start-CapturedChild {
    param($Name, $FilePath, $Arguments, $Environment, $WorkingDirectory, $StdoutPath, $StderrPath)
    $script:calls.Add($Name)
    Assert-Test ($Arguments -join ' ' -ceq 'local_russh_smoke_fixture_server --ignored --exact --nocapture') 'Fixture must run its exact ignored test.'
    foreach ($path in @($Environment.RSHELL_QA_SSH_SMOKE_ENCRYPTED_KEY_PATH, "$($Environment.RSHELL_QA_SSH_SMOKE_ENCRYPTED_KEY_PATH).pub", $Environment.RSHELL_QA_SSH_SMOKE_AGENT_PUBLIC_KEY_PATH, $Environment.RSHELL_QA_SSH_SMOKE_OBSERVATION_DIR)) {
        Assert-Test (Test-Path -LiteralPath $path) 'Fixture file prerequisite missing.'
    }
    foreach ($envName in @('RSHELL_QA_SSH_SMOKE_PASSWORD_ENV', 'RSHELL_QA_SSH_SMOKE_KEY_PASSPHRASE_ENV', 'RSHELL_QA_SSH_SMOKE_KBI_VISIBLE_ANSWER_ENV', 'RSHELL_QA_SSH_SMOKE_KBI_ONE_TIME_CODE_ENV')) {
        Assert-Test (-not [string]::IsNullOrEmpty($Environment[$Environment[$envName]])) 'Fixture secret prerequisite missing.'
    }
    Assert-Test ($Environment.RSHELL_QA_SSH_SMOKE_RUN_NONCE -ceq $runId) 'Fixture nonce missing.'
    Assert-Test ($Environment.RSHELL_QA_SSH_SMOKE_EXPECTED_SURFACES -ceq 'native_password,native_key,native_keyboard_interactive,system_agent,host_key') 'Fixture surface coverage weakened.'
    $isNonzero = $Name -ceq 'fixture_nonzero_shutdown'
    $marker = if ($isNonzero) { 'intentional fixture final assertions failure after exact server shutdown' } else { 'intentional fail_before_fixture_ready before server mutation' }
    Assert-Test ($Environment[$(if ($isNonzero) { 'RSHELL_QA_INJECT_FINAL_FAILURE' } else { 'RSHELL_QA_INJECT_FAIL_BEFORE_READY' })] -ceq '1') 'Fault injection missing.'
    if ($isNonzero -and $script:case -eq 'fixture-start-failure') { throw 'mock fixture start failed' }
    if (($isNonzero -and $script:case -notin @('exit-before-ready', 'ready-timeout')) -or $script:case -eq 'early-ready') {
        Write-Utf8File $Environment.RSHELL_QA_SSH_SMOKE_READY_PATH ((@{
                    version = 1; generated_by = 'p0_qa'; run_nonce = $runId
                    fixture = $(if ($script:case -eq 'wrong-ready-binding') { 'stale-fixture' } else { $Environment.RSHELL_QA_SSH_SMOKE_FIXTURE_ID })
                }) | ConvertTo-Json)
    }
    if ($script:case -eq 'wrong-shutdown-fault' -and $isNonzero) { $marker = 'unrelated server startup failure' }
    if ($script:case -eq 'wrong-before-ready-fault' -and -not $isNonzero) { $marker = 'unrelated fixture startup failure' }
    $output = New-TestOutput 'local_russh_smoke_fixture_server' $marker
    $id = 1000 + $script:calls.Count
    $process = [pscustomobject]@{
        Id = $id; ExitCode = $(if ($script:case -eq 'fixture-zero' -or ($script:case -eq 'nonzero-zero' -and $isNonzero)) { 0 } else { 101 })
        HasExited = ($script:case -eq 'exit-before-ready'); WaitCount = 0; Killed = $false; Disposed = $false
        IsNonzero = $isNonzero; StopPath = $Environment.RSHELL_QA_SSH_SMOKE_STOP_PATH
    }
    $process | Add-Member ScriptMethod WaitForExit {
        param($Milliseconds)
        $this.WaitCount++
        Assert-Test ($Milliseconds -gt 0 -and $Milliseconds -le 30000) 'Wait/reap must remain bounded.'
        if ($this.IsNonzero -and $script:case -in @('child-timeout', 'unconfirmed-reap')) {
            if ($this.WaitCount -eq 1 -or $script:case -eq 'unconfirmed-reap') { return $false }
        }
        if ($this.IsNonzero -and $script:case -eq 'happy') {
            Assert-Test (Test-Path -LiteralPath $this.StopPath) 'Nonzero shutdown must follow this run stop.'
        }
        return $true
    }
    $process | Add-Member ScriptMethod Kill { param($Tree) $this.Killed = $true }
    $process | Add-Member ScriptMethod Dispose { $this.Disposed = $true }
    $script:ownedChildIds.Add($id)
    $run = [pscustomobject]@{
        Name = $Name; Process = $process; Completed = $false
        StdoutTask = [System.Threading.Tasks.Task]::FromResult([string]$output[0])
        StderrTask = [System.Threading.Tasks.Task]::FromResult([string]$output[1])
        StdoutPath = $StdoutPath; StderrPath = $StderrPath
    }
    if ($script:case -eq 'capture-unconfirmed' -and $isNonzero) {
        $task = [pscustomobject]@{}
        $task | Add-Member ScriptMethod Wait { param($Milliseconds) return $false }
        $run.StdoutTask = $task
    }
    $script:runs.Add($run)
    return $run
}

function Invoke-ModeCase {
    param([string]$Mode, [string]$Case)
    $script:case = $Case
    $script:calls = [System.Collections.Generic.List[string]]::new()
    $script:cleanupReferences = [System.Collections.Generic.List[string]]::new()
    $script:ownedChildIds = [System.Collections.Generic.List[int]]::new()
    $script:runs = [System.Collections.Generic.List[object]]::new()
    $script:phases = [System.Collections.Generic.List[object]]::new()
    $script:mutated = $false
    $runId = [Guid]::NewGuid().ToString('N')
    $tempRoot = Join-Path $testRoot $runId
    $repoRoot = Join-Path $tempRoot 'repo'
    [void][System.IO.Directory]::CreateDirectory((Join-Path $repoRoot 'target/debug/deps'))
    $artifactRoot = Join-Path $tempRoot 'logs'
    [void][System.IO.Directory]::CreateDirectory($artifactRoot)
    $fixtureObservationRoot = Join-Path $tempRoot 'observations'
    [void][System.IO.Directory]::CreateDirectory($fixtureObservationRoot)
    $binarySuffix = ''
    $platformIsWindows = $false
    $baseEnvironment = @{}
    $cargo = 'mock-cargo'; $sshKeygen = 'mock-keygen'; $stem = 'test'
    $fixtureRun = $null; $fixtureFailureRun = $null; $fixtureNonzeroRun = $null
    $fixtureStop = Join-Path $tempRoot 'fixture.stop'
    $agentPublicKey = Join-Path $tempRoot 'parent-agent-key.pub'
    Write-Utf8File $agentPublicKey 'mock agent public key'
    Write-Utf8File (Join-Path $repoRoot 'target/debug/deps/ssh_smoke-0123') 'mock fixture'
    $vaultReference = "rshell://credential/$runId"
    $vaultFailureReference = "rshell://credential/$runId-failure"
    $vaultCleanupLedger = Join-Path $tempRoot 'vault-cleanup-ledger.json'
    $vaultCleanupRequired = $false
    $failure = $null
    $diagnostics = [System.Collections.Generic.List[string]]::new()
    $directObservation = @{ vault = Join-Path $tempRoot 'vault.json' }
    . ([scriptblock]::Create($secrets))
    $childEnvironment = $secretEnvironment.Clone()
    foreach ($flag in $flags) { . ([scriptblock]::Create($flag.Extent.Text)) }
    $guiAllowed = . ([scriptblock]::Create($gui[0].Clauses[0].Item1.Extent.Text))
    Assert-Test ($guiAllowed -eq ($Mode -in @('Gtk', 'All'))) 'Ssh must not enter the production GTK branch; All must retain it.'
    try {
        foreach ($block in $blocks) {
            $output = . ([scriptblock]::Create($block.Extent.Text))
            foreach ($line in @($output)) { if ($null -ne $line) { $diagnostics.Add([string]$line) } }
        }
    }
    catch { $failure = $_.Exception.Message }
    finally {
        foreach ($block in $cleanup) { . ([scriptblock]::Create($block.Extent.Text)) }
    }
    if ($Case -eq 'happy') {
        Assert-Test ($null -eq $failure) "Mode $Mode failed: $failure"
        $expected = if ($Mode -in @('Ssh', 'All')) { 1 } else { 0 }
        foreach ($name in @('fail_during_vault_probe', 'fail_before_fixture_ready', 'fixture_nonzero_shutdown')) {
            Assert-Test (@($script:calls | Where-Object { $_ -ceq $name }).Count -eq $expected) "Mode $Mode must execute $name exactly $expected time(s)."
            Assert-Test (@($diagnostics | Where-Object { $_.StartsWith("P0_FAILURE_PROBE name=$name ") }).Count -eq $expected) 'Verified failure evidence must be visible exactly once.'
        }
        Assert-Test (@($script:calls | Where-Object { $_ -ceq 'vault-real-os' }).Count -eq $(if ($needsVault) { 1 } else { 0 })) 'Positive vault mode scope changed.'
        Assert-Test (-not ($script:calls -contains 'gtk-production')) 'Mode-path regression must never launch GTK.'
    }
    else {
        Assert-Test ($null -ne $failure) "Invalid probe accepted: $Case"
        Assert-Test (@($script:phases | Where-Object { $_.name -ceq 'fixture_nonzero_shutdown' }).Count -eq 0) "Failed probe was reported as passed: $Case"
    }
    if ($vaultCleanupRequired) {
        Assert-Test ($script:cleanupReferences -contains $vaultFailureReference) 'Finally lost Ssh vault failure cleanup.'
        Assert-Test ($script:cleanupReferences -contains $vaultReference) 'Finally lost the parent exact-reference ledger.'
    }
    if ($Case -notin @('unconfirmed-reap', 'capture-unconfirmed')) {
        Assert-Test ($script:ownedChildIds.Count -eq 0) "Owned child not retired: $Case"
        foreach ($run in $script:runs) { Assert-Test ($run.Completed -and $run.Process.Disposed) 'Fallback must confirm and retire its owned child.' }
    }
    else { Assert-Test ($script:ownedChildIds.Count -eq 1) 'Unconfirmed child/capture must remain owned, never claim cleanup.' }
}

$temporaryBase = [System.IO.Path]::GetTempPath()
if (-not (Test-Path -LiteralPath $temporaryBase -PathType Container)) { throw 'Test temporary parent unavailable.' }
$testRoot = Join-Path $temporaryBase "rshell-p0-failure-test-$([Guid]::NewGuid().ToString('N'))"
[void][System.IO.Directory]::CreateDirectory($testRoot)
$savedDisplay = $env:DISPLAY
try {
    $env:DISPLAY = $null
    foreach ($mode in @('Ssh', 'All', 'Unit', 'Gtk', 'Vault')) { Invoke-ModeCase $mode 'happy' }
    $negativeCases = @('vault-zero', 'vault-startup-error', 'vault-wrong-test', 'vault-duplicate-test', 'vault-cleanup-nonzero', 'vault-cleanup-zero-tests', 'fixture-zero', 'nonzero-zero', 'early-ready', 'fixture-start-failure', 'exit-before-ready', 'ready-timeout', 'wrong-ready-binding', 'wrong-before-ready-fault', 'wrong-shutdown-fault', 'child-timeout', 'unconfirmed-reap', 'capture-unconfirmed')
    foreach ($mode in @('Ssh', 'All')) {
        foreach ($case in $negativeCases) { Invoke-ModeCase $mode $case }
    }
    "P0_FAILURE_PROBES_MOCK_PASS modes=5 shared_probes=3 negative_cases=$($negativeCases.Count * 2) gtk_started=0 vault_mutations=mocked"
}
finally {
    $env:DISPLAY = $savedDisplay
    # Exact, GUID-owned fixture root only; no native/vault/agent cleanup is used.
    if ([System.IO.Path]::GetDirectoryName($testRoot) -ne $temporaryBase.TrimEnd([System.IO.Path]::DirectorySeparatorChar) -or
        [System.IO.Path]::GetFileName($testRoot) -notmatch '^rshell-p0-failure-test-[0-9a-f]{32}$') { throw 'Unsafe test cleanup root.' }
    [System.IO.Directory]::Delete($testRoot, $true)
}
