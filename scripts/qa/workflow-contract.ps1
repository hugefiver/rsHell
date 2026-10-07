param(
    [string]$CiPath = (Join-Path $PSScriptRoot "..\..\.github\workflows\ci.yml"),
    [string]$ReleasePath = (Join-Path $PSScriptRoot "..\..\.github\workflows\release.yml"),
    [string]$P0Path = (Join-Path $PSScriptRoot "p0-smoke.ps1"),
    [string]$PackagePath = (Join-Path $PSScriptRoot "assert-package.ps1"),
    [string]$DisplayCoordinatorPath = (Join-Path $PSScriptRoot "windows-display-experiment.ps1"),
    [AllowEmptyString()][string]$CiText = "",
    [AllowEmptyString()][string]$ReleaseText = "",
    [AllowEmptyString()][string]$P0Text = "",
    [AllowEmptyString()][string]$PackageText = "",
    [AllowEmptyString()][string]$DisplayCoordinatorText = "",
    [ValidateSet("", "dead-workspace-gate", "missing-workspace-display-setup", "undersized-workspace-display", "skipped-native-workspace-test", "conditional-workspace-display-restore", "missing-workspace-display-restore", "mismatched-workspace-display-ledger", "missing-workspace-display-restore-check", "restore-before-windows-p0", "missing-workspace-display-started-handoff", "early-workspace-display-restore-exit", "old-p0-fullscreen-display-helper", "repeated-p0-display-setup", "nested-p0-display-ledger", "display-restore-in-vault-cleanup", "missing-workspace-display-observation", "missing-p0-display-observation", "missing-workarea-baseline", "missing-workarea-restore", "missing-workarea-baseline-marker", "missing-workarea-restore-marker", "dead-terminal-engine-gate", "conditional-terminal-engine-gate", "continue-terminal-engine-gate", "missing-terminal-engine-gate", "duplicate-terminal-engine-gate", "misplaced-terminal-engine-gate", "skipped-p0-gate", "conditional-p0-gate", "continued-p0-gate", "linux-ssh-p0", "windows-ssh-p0", "macos-all-p0", "missing-macos-p0", "duplicate-macos-p0", "conditional-macos-p0", "nested-conditional-macos-p0", "missing-linux-vault", "missing-macos-vault", "missing-windows-vault", "missing-linux-vault-cleanup", "missing-macos-vault-cleanup", "missing-windows-vault-cleanup", "missing-macos-gui-skip", "missing-fatal-gtk-warnings", "missing-package-startup-field", "missing-platform-matrix-member", "weakened-cleanup-secret-ordering")]
    [string]$RegressionProbe = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Read-WorkflowText {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Label
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label workflow is missing."
    }

    return [System.IO.File]::ReadAllText((Resolve-Path -LiteralPath $Path).Path)
}

function Add-ContractFailure {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures,
        [Parameter(Mandatory)][string]$Message
    )

    $Failures.Add($Message)
}

function Assert-Exactly {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Pattern,
        [Parameter(Mandatory)][int]$Expected,
        [Parameter(Mandatory)][string]$Label,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    $actual = [regex]::Matches($Text, $Pattern, [System.Text.RegularExpressions.RegexOptions]::Multiline).Count
    if ($actual -ne $Expected) {
        Add-ContractFailure -Failures $Failures -Message "$Label must occur exactly $Expected time(s); found $actual."
    }
}

function Assert-Contains {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Pattern,
        [Parameter(Mandatory)][string]$Label,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    if (-not [regex]::IsMatch($Text, $Pattern, [System.Text.RegularExpressions.RegexOptions]::Multiline)) {
        Add-ContractFailure -Failures $Failures -Message "$Label is missing."
    }
}

function Assert-Absent {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Pattern,
        [Parameter(Mandatory)][string]$Label,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    if ([regex]::IsMatch($Text, $Pattern, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)) {
        Add-ContractFailure -Failures $Failures -Message "$Label must not be present."
    }
}

function Assert-OnlyPowerShellShells {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Label,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    $overrides = [regex]::Matches($Text, "(?m)^[ \t]*shell:[ \t]*(?<value>[^\r\n#]+)")
    foreach ($override in $overrides) {
        if ($override.Groups["value"].Value.Trim() -cne "pwsh") {
            Add-ContractFailure -Failures $Failures -Message "$Label must not use a non-PowerShell shell override."
        }
    }
}

function Get-NamedStepBlocks {
    param([Parameter(Mandatory)][string]$Text)

    return @([regex]::Matches(
            $Text,
            "(?ms)^ {6}- name: (?<name>[^\r\n]+)\r?\n(?<body>.*?)(?=^ {6}- (?:name:|uses:)|\z)"
        ))
}

function Assert-NamedStep {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    $matches = @(Get-NamedStepBlocks -Text $Text | Where-Object { $_.Groups["name"].Value -ceq $Name })
    if ($matches.Count -ne 1) {
        Add-ContractFailure -Failures $Failures -Message "Workflow step '$Name' must occur exactly once; found $($matches.Count)."
        return $null
    }
    return $matches[0].Groups["body"].Value
}

function Assert-StepHasNoYamlCondition {
    param(
        [AllowNull()][string]$Step,
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    if ($null -ne $Step -and [regex]::IsMatch($Step, "(?m)^ {8}if:\s*")) {
        Add-ContractFailure -Failures $Failures -Message "Workflow step '$Name' must be unconditional."
    }
}

function Assert-StepLine {
    param(
        [AllowNull()][string]$Step,
        [Parameter(Mandatory)][string]$Line,
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    if ($null -eq $Step -or -not [regex]::IsMatch($Step, "(?m)^ {10,}$([regex]::Escape($Line))\s*$")) {
        Add-ContractFailure -Failures $Failures -Message "Workflow step '$Name' is missing required command."
    }
}

function Assert-StepLineCount {
    param(
        [AllowNull()][string]$Step,
        [Parameter(Mandatory)][string]$Line,
        [Parameter(Mandatory)][int]$Expected,
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    $actual = if ($null -eq $Step) { 0 } else { [regex]::Matches($Step, "(?m)^ {10,}$([regex]::Escape($Line))\s*$").Count }
    if ($actual -ne $Expected) {
        Add-ContractFailure -Failures $Failures -Message "Workflow step '$Name' must contain $Expected required failure check(s); found $actual."
    }
}

function Assert-StepPattern {
    param(
        [AllowNull()][string]$Step,
        [Parameter(Mandatory)][string]$Pattern,
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    if ($null -eq $Step -or -not [regex]::IsMatch($Step, $Pattern, [System.Text.RegularExpressions.RegexOptions]::Multiline)) {
        Add-ContractFailure -Failures $Failures -Message "Workflow step '$Name' is missing a required fail-closed operation."
    }
}

function Assert-TerminalEngineGateStep {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Workflow,
        [Parameter(Mandatory)][string]$FailureCheck,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    $name = "Run terminal engine gate"
    $matches = @(Get-NamedStepBlocks -Text $Text | Where-Object { $_.Groups["name"].Value -ceq $name })
    if ($matches.Count -ne 1) {
        Add-ContractFailure -Failures $Failures -Message "$Workflow workflow step '$name' must occur exactly once; found $($matches.Count)."
        return $null
    }

    $step = $matches[0]
    $body = $step.Groups["body"].Value
    $command = "pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1"
    Assert-StepHasNoYamlCondition -Step $body -Name "$Workflow $name" -Failures $Failures
    Assert-StepPattern -Step $body -Pattern "(?m)^ {8}run: \|\s*$" -Name "$Workflow $name" -Failures $Failures
    Assert-StepLineCount -Step $body -Line $command -Expected 1 -Name "$Workflow $name" -Failures $Failures
    Assert-StepLineCount -Step $body -Line $FailureCheck -Expected 1 -Name "$Workflow $name" -Failures $Failures
    $commandAndFailure = "(?m)^ {10}$([regex]::Escape($command))\r?\n {10}$([regex]::Escape($FailureCheck))\s*$"
    if (-not [regex]::IsMatch($body, $commandAndFailure)) {
        Add-ContractFailure -Failures $Failures -Message "$Workflow workflow step '$name' must immediately fail on a nonzero terminal-engine gate exit."
    }
    return $step
}

function Assert-UnconditionalSmokeCommand {
    param([string]$Step, [string]$Name, $Failures)

    if ($null -eq $Step) { return }
    $run = [regex]::Match($Step, '(?ms)^ {8}run: \|\r?\n(?<script>.*)$').Groups['script'].Value
    $run = [regex]::Replace($run, '(?m)^ {10}', '')
    $tokens = $null
    $errors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseInput($run, [ref]$tokens, [ref]$errors)
    if ($errors.Count -ne 0) {
        Add-ContractFailure $Failures "CI step '$Name' has an invalid PowerShell run block."
        return
    }
    # Linux runs its smoke in the literal dbus-session script, not the parent.
    $inner = @($ast.FindAll({ param($node)
                $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and
                $node.Left.Extent.Text -ceq '$inner'
            }, $true))
    if ($inner.Count -eq 1) {
        $literal = @($inner[0].Right.FindAll({ param($node)
                    $node -is [System.Management.Automation.Language.StringConstantExpressionAst]
                }, $true))
        if ($literal.Count -ne 1) {
            Add-ContractFailure $Failures "CI step '$Name' requires one literal session script."
            return
        }
        $ast = [System.Management.Automation.Language.Parser]::ParseInput(
            $literal[0].Value, [ref]$tokens, [ref]$errors)
    }
    $commands = @($ast.FindAll({ param($node)
                $node -is [System.Management.Automation.Language.CommandAst] -and
                $node.Extent.Text -match 'p0-smoke\.ps1'
            }, $true))
    if ($errors.Count -ne 0 -or $commands.Count -ne 1) {
        Add-ContractFailure $Failures "CI step '$Name' must execute exactly one smoke command."
        return
    }
    for ($parent = $commands[0].Parent; $null -ne $parent; $parent = $parent.Parent) {
        if ($parent -is [System.Management.Automation.Language.IfStatementAst] -or
            $parent -is [System.Management.Automation.Language.LoopStatementAst] -or
            $parent -is [System.Management.Automation.Language.FunctionDefinitionAst] -or
            $parent -is [System.Management.Automation.Language.ScriptBlockExpressionAst]) {
            Add-ContractFailure $Failures "CI step '$Name' must not condition or defer its smoke command."
        }
    }
}

function Assert-ReadOnlyDisplayObservation {
    param([string]$Step, [string]$Phase, [string]$Gate, [string]$Name, $Failures)

    $commands = @(
        '. scripts/qa/windows-display-experiment.ps1',
        '$displayMode = Get-WorkspaceDisplayCurrent',
        '$displayWorkArea = Get-WorkspaceDisplayWorkArea',
        '$displayTarget = Get-WorkspaceDisplayPrimaryMonitorRect',
        ('Write-Output "RSHELL_WORKSPACE_DISPLAY_CURRENT phase=' + $Phase + ' target_width=1920 target_height=1080 mode=$($displayMode | ConvertTo-Json -Compress) workarea=$($displayWorkArea | ConvertTo-Json -Compress) target=$($displayTarget | ConvertTo-Json -Compress)"')
    )
    foreach ($command in $commands) {
        Assert-StepLineCount -Step $Step -Line $command -Expected 1 -Name $Name -Failures $Failures
    }
    $pattern = ($commands | ForEach-Object { '(?m)^ {12}' + [regex]::Escape($_) + '\r?\n' }) -join ''
    Assert-StepPattern -Step $Step -Pattern $pattern -Name "$Name read-only display observation" -Failures $Failures
    if ($Step.IndexOf($commands[-1], [System.StringComparison]::Ordinal) -ge $Step.IndexOf($Gate, [System.StringComparison]::Ordinal)) {
        Add-ContractFailure $Failures "$Name must query and log actual mode, workarea and primary target before its gate."
    }
    Assert-Absent -Text $Step -Pattern 'Invoke-WorkspaceDisplay(?:Setup|Restore|Child)|(?:Set|Reset)-WorkspaceDisplay|Set-WorkspaceWorkArea|Write-DisplayNewFile|Read-Display(?:WorkArea)?Baseline' -Label "$Name repeated display mutation/baseline" -Failures $Failures
}

function Get-NamedStepBlock {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][string]$Name
    )

    return @(Get-NamedStepBlocks -Text $Text | Where-Object { $_.Groups["name"].Value -ceq $Name })
}

function Assert-P0CleanupAndSecretOrder {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Failures
    )

    $ownedCleanup = $Text.LastIndexOf('Add-Phase "owned_process_cleanup"', [System.StringComparison]::Ordinal)
    $secretScan = $Text.LastIndexOf('-Name "assert-no-secrets"', [System.StringComparison]::Ordinal)
    $temporaryCleanup = $Text.LastIndexOf('Remove-Item -LiteralPath $tempRoot -Recurse -Force', [System.StringComparison]::Ordinal)
    $finalization = $Text.LastIndexOf('$finalizeRoot = Join-Path', [System.StringComparison]::Ordinal)
    if ($ownedCleanup -lt 0 -or $secretScan -lt 0 -or $temporaryCleanup -lt 0 -or $finalization -lt 0 -or
        -not ($ownedCleanup -lt $secretScan -and $secretScan -lt $temporaryCleanup -and $temporaryCleanup -lt $finalization)) {
        Add-ContractFailure -Failures $Failures -Message "P0 cleanup, secret scan, and artifact finalization must remain fail-closed and ordered."
    }
}

$ci = if ($CiText.Length -gt 0) { $CiText } else { Read-WorkflowText -Path $CiPath -Label "CI" }
$release = if ($ReleaseText.Length -gt 0) { $ReleaseText } else { Read-WorkflowText -Path $ReleasePath -Label "Release" }
$p0 = if ($P0Text.Length -gt 0) { $P0Text } else { Read-WorkflowText -Path $P0Path -Label "P0 smoke" }
$package = if ($PackageText.Length -gt 0) { $PackageText } else { Read-WorkflowText -Path $PackagePath -Label "Package assertion" }
$displayCoordinator = if ($DisplayCoordinatorText.Length -gt 0) { $DisplayCoordinatorText } else { Read-WorkflowText -Path $DisplayCoordinatorPath -Label "Display coordinator" }

if ($RegressionProbe.Length -gt 0) {
    $probeCi = $ci
    $probeP0 = $p0
    $probePackage = $package
    $probeDisplayCoordinator = $displayCoordinator
    switch ($RegressionProbe) {
        "dead-workspace-gate" {
            $stepHeader = "      - name: Run required workspace gates"
            $probeCi = $ci.Replace($stepHeader, "$stepHeader`n        if: false")
        }
        "missing-workspace-display-setup" {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Prepare workspace display (Windows)")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate workspace display setup." }
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length)
        }
        "undersized-workspace-display" {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Prepare workspace display (Windows)")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate workspace display setup." }
            $undersized = $steps[0].Value.Replace('-RunnerTemp $env:RUNNER_TEMP -Width 1920 -Height 1080', '-RunnerTemp $env:RUNNER_TEMP -Width 1600 -Height 900')
            if ($undersized -ceq $steps[0].Value) { throw "Workflow regression probe could not lower workspace display resolution." }
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length).Insert($steps[0].Index, $undersized)
        }
        "skipped-native-workspace-test" {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Run required workspace gates")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate workspace gates." }
            $skipped = $steps[0].Value.Replace('cargo test --workspace --all-features --locked', 'cargo test --workspace --all-features --locked -- --skip actor_panic_keeps_realized_main_window_alive')
            if ($skipped -ceq $steps[0].Value) { throw "Workflow regression probe could not filter native workspace tests." }
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length).Insert($steps[0].Index, $skipped)
        }
        "conditional-workspace-display-restore" {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Restore workspace display (Windows)")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate workspace display restore." }
            $conditional = [regex]::Replace($steps[0].Value, "(?m)^ {8}if: always\(\) && runner\.os == 'Windows'", "        if: runner.os == 'Windows'")
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length).Insert($steps[0].Index, $conditional)
        }
        "missing-workspace-display-restore" {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Restore workspace display (Windows)")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate workspace display restore." }
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length)
        }
        "mismatched-workspace-display-ledger" {
            $probeCi = $ci.Replace('Invoke-WorkspaceDisplayRestore -Root $displayRoot -RunnerTemp $env:RUNNER_TEMP -Ledger $displayLedger', 'Invoke-WorkspaceDisplayRestore -Root $displayRoot -RunnerTemp $env:RUNNER_TEMP -Ledger $otherLedger')
        }
        "missing-workspace-display-restore-check" {
            $probeCi = $ci.Replace("catch { [Console]::Error.WriteLine('Windows workspace display restoration failed; owned state retained.'); exit 1 }", 'catch { Write-Output "Restore result unchecked" }')
        }
        "restore-before-windows-p0" {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Restore workspace display (Windows)")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate display restore." }
            $withoutRestore = $ci.Remove($steps[0].Index, $steps[0].Length)
            $p0Steps = @(Get-NamedStepBlock -Text $withoutRestore -Name "Run Credential Manager vault probe and P0 All smoke (Windows)")
            if ($p0Steps.Count -ne 1) { throw "Workflow regression probe could not locate Windows P0." }
            $probeCi = $withoutRestore.Insert($p0Steps[0].Index, $steps[0].Value)
        }
        "missing-workspace-display-started-handoff" {
            $probeCi = $ci.Replace(' -Started $env:RSHELL_WORKSPACE_DISPLAY_STARTED', '')
        }
        "early-workspace-display-restore-exit" {
            $probeCi = $ci.Replace('          $displayRoot = $env:RSHELL_WORKSPACE_DISPLAY_ROOT', '          if ([string]::IsNullOrWhiteSpace($env:RSHELL_WORKSPACE_DISPLAY_ROOT)) { exit 0 }' + "`n" + '          $displayRoot = $env:RSHELL_WORKSPACE_DISPLAY_ROOT')
        }
        { $_ -in @('old-p0-fullscreen-display-helper', 'repeated-p0-display-setup', 'nested-p0-display-ledger', 'display-restore-in-vault-cleanup', 'missing-p0-display-observation') } {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Run Credential Manager vault probe and P0 All smoke (Windows)")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate Windows P0." }
            $original = $steps[0].Value
            $changed = switch ($RegressionProbe) {
                'old-p0-fullscreen-display-helper' { $original.Replace('            $displayMode = Get-WorkspaceDisplayCurrent', '            pwsh -NoProfile -File scripts/qa/windows-display.ps1 -Mode Apply -Ledger $displayLedger -Width 1920 -Height 1080') }
                'repeated-p0-display-setup' { $original.Replace('            $displayMode = Get-WorkspaceDisplayCurrent', '            Invoke-WorkspaceDisplaySetup -RunnerTemp $env:RUNNER_TEMP -Width 1920 -Height 1080') }
                'nested-p0-display-ledger' { $original.Replace('          try {', '          $displayLedger = Join-Path $vaultRoot "display-mode.json"' + "`n" + '          try {') }
                'display-restore-in-vault-cleanup' { $original.Replace('          finally {', '          finally {' + "`n" + '            Invoke-WorkspaceDisplayRestore -Root $displayRoot -RunnerTemp $env:RUNNER_TEMP -Ledger $displayLedger -Started $env:RSHELL_WORKSPACE_DISPLAY_STARTED') }
                'missing-p0-display-observation' { $original.Replace('            $displayWorkArea = Get-WorkspaceDisplayWorkArea', '            $displayWorkArea = $displayTarget') }
            }
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length).Insert($steps[0].Index, $changed)
        }
        "missing-workspace-display-observation" {
            $steps = @(Get-NamedStepBlock -Text $ci -Name "Run required workspace gates")
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate workspace gates." }
            $changed = $steps[0].Value.Replace('            $displayMode = Get-WorkspaceDisplayCurrent', '            $displayMode = $displayTarget')
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length).Insert($steps[0].Index, $changed)
        }
        "missing-workarea-baseline" {
            $probeDisplayCoordinator = $displayCoordinator.Replace('Write-DisplayNewFile (Join-Path $displayRoot ''workarea.json'') ($workAreaBaseline | ConvertTo-Json -Compress)', 'Write-Output "workarea baseline omitted"')
        }
        "missing-workarea-restore" {
            $probeDisplayCoordinator = $displayCoordinator.Replace('$child = Invoke-WorkspaceDisplayChild $Root (Join-Path $Root ''workarea.json'') ''Restore'' ''Dynamic'' $WorkAreaBaseline -Operation Workarea', '$child = $WorkAreaBaseline')
        }
        "missing-workarea-baseline-marker" {
            $probeDisplayCoordinator = $displayCoordinator.Replace('Write-DisplayRectObservation ''baseline'' ''setup'' $workAreaBaseline', 'Write-Output "workarea baseline marker omitted"')
        }
        "missing-workarea-restore-marker" {
            $probeDisplayCoordinator = $displayCoordinator.Replace('Write-DisplayRectObservation ''restore_after_exit'' $Arm $current', 'Write-Output "workarea restore marker omitted"')
        }
        "dead-terminal-engine-gate" {
            $stepHeader = "      - name: Run terminal engine gate"
            $probeCi = $ci.Replace($stepHeader, "$stepHeader`n        if: false")
        }
        "conditional-terminal-engine-gate" {
            $stepHeader = "      - name: Run terminal engine gate"
            $probeCi = $ci.Replace($stepHeader, "$stepHeader`n        if: runner.os == 'Windows'")
        }
        "continue-terminal-engine-gate" {
            $stepHeader = "      - name: Run terminal engine gate"
            $probeCi = $ci.Replace($stepHeader, "$stepHeader`n        continue-on-error: true")
        }
        "missing-terminal-engine-gate" {
            $gateMatches = @(Get-NamedStepBlock -Text $ci -Name "Run terminal engine gate")
            if ($gateMatches.Count -ne 1) { throw "Workflow regression probe could not locate the terminal-engine gate." }
            $probeCi = $ci.Remove($gateMatches[0].Index, $gateMatches[0].Length)
        }
        "duplicate-terminal-engine-gate" {
            $gateMatches = @(Get-NamedStepBlock -Text $ci -Name "Run terminal engine gate")
            if ($gateMatches.Count -ne 1) { throw "Workflow regression probe could not locate the terminal-engine gate." }
            $probeCi = $ci.Insert($gateMatches[0].Index, $gateMatches[0].Value)
        }
        "misplaced-terminal-engine-gate" {
            $gateMatches = @(Get-NamedStepBlock -Text $ci -Name "Run terminal engine gate")
            if ($gateMatches.Count -ne 1) { throw "Workflow regression probe could not locate the terminal-engine gate." }
            $withoutGate = $ci.Remove($gateMatches[0].Index, $gateMatches[0].Length)
            $workspaceMatches = @(Get-NamedStepBlock -Text $withoutGate -Name "Run required workspace gates")
            if ($workspaceMatches.Count -ne 1) { throw "Workflow regression probe could not locate workspace gates." }
            $probeCi = $withoutGate.Insert($workspaceMatches[0].Index, $gateMatches[0].Value)
        }
        "skipped-p0-gate" {
            $command = "pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All"
            $probeCi = $ci.Replace($command, 'Write-Output "P0 All gate skipped"')
        }
        "conditional-p0-gate" {
            $command = "pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All"
            $probeCi = $ci.Replace($command, "if (`$true) { $command }")
        }
        "continued-p0-gate" {
            $stepHeader = "      - name: Run Secret Service vault probe and P0 All smoke (Linux)"
            $probeCi = $ci.Replace($stepHeader, "$stepHeader`n        continue-on-error: true")
        }
        { $_ -in @("linux-ssh-p0", "windows-ssh-p0", "macos-all-p0", "missing-macos-p0", "duplicate-macos-p0", "conditional-macos-p0", "nested-conditional-macos-p0", "missing-linux-vault", "missing-macos-vault", "missing-windows-vault", "missing-linux-vault-cleanup", "missing-macos-vault-cleanup", "missing-windows-vault-cleanup", "missing-macos-gui-skip") } {
            $platform = if ($_ -match 'linux') { 'Linux' } elseif ($_ -match 'windows') { 'Windows' } else { 'macOS' }
            $steps = @(Get-NamedStepBlocks -Text $ci | Where-Object { $_.Groups['name'].Value -match "vault probe and P0 .* smoke \($platform\)$" })
            if ($steps.Count -ne 1) { throw "Workflow regression probe could not locate its platform smoke." }
            $original = $steps[0].Value
            $command = 'pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode ' + $(if ($platform -eq 'macOS') { 'Ssh' } else { 'All' })
            $changed = switch ($RegressionProbe) {
                { $_ -in @('linux-ssh-p0', 'windows-ssh-p0') } { $original.Replace($command, $command.Replace('-Mode All', '-Mode Ssh')) }
                'macos-all-p0' { $original.Replace($command, $command.Replace('-Mode Ssh', '-Mode All')) }
                'missing-macos-p0' { $original.Replace($command, 'Write-Output "smoke removed"') }
                'duplicate-macos-p0' { $original.Replace($command, "$command`n            $command") }
                'conditional-macos-p0' { $original.Replace($command, "if (`$true) { $command }") }
                'nested-conditional-macos-p0' { $original.Replace($command, "if (`$true) {`n            $command`n            }") }
                'missing-macos-gui-skip' { $original.Replace('P0_NATIVE_GUI_SKIP platform=macos', 'GUI_UNVERIFIED') }
                { $_ -match 'vault-cleanup$' } { $original.Replace('system_vault_cleanup_exact_parent_reference', 'missing_cleanup') }
                { $_ -match 'vault$' } { $original.Replace('system_vault_real_os_probe_uses_coordinator_and_cleans_random_entry', 'missing_probe') }
            }
            $probeCi = $ci.Remove($steps[0].Index, $steps[0].Length).Insert($steps[0].Index, $changed)
        }
        "missing-fatal-gtk-warnings" {
            $probeP0 = $p0.Replace('G_DEBUG = "fatal-warnings"', 'G_DEBUG = "warnings"')
        }
        "missing-package-startup-field" {
            $probePackage = $package.Replace('"measured_terminal_geometry_ready",', '"missing_startup_field",')
        }
        "missing-platform-matrix-member" {
            $probeCi = [regex]::Replace(
                $ci,
                '(?m)^          - name: macOS arm64\r?\n            os: macos-26\r?\n?',
                '',
                1
            )
        }
        "weakened-cleanup-secret-ordering" {
            $probeP0 = "$p0`nAdd-Phase `"owned_process_cleanup`""
        }
    }
    if ($probeCi -ceq $ci -and $probeP0 -ceq $p0 -and $probePackage -ceq $package -and $probeDisplayCoordinator -ceq $displayCoordinator) {
        throw "Workflow regression probe could not mutate its contract input."
    }
    $temporaryRoot = [System.IO.Path]::GetTempPath()
    if (-not (Test-Path -LiteralPath $temporaryRoot -PathType Container)) {
        throw "Workflow regression probe temporary directory is unavailable."
    }
    $probeToken = [Guid]::NewGuid().ToString('N')
    $probeCiPath = Join-Path $temporaryRoot "rshell-workflow-contract-$probeToken.yml"
    $probeP0Path = Join-Path $temporaryRoot "rshell-workflow-contract-$probeToken.ps1"
    $probePackagePath = Join-Path $temporaryRoot "rshell-workflow-contract-$probeToken-package.ps1"
    $probeDisplayCoordinatorPath = Join-Path $temporaryRoot "rshell-workflow-contract-$probeToken-display.ps1"
    $pwsh = (Get-Command -Name "pwsh" -ErrorAction Stop).Source
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $pwsh
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in @("-NoProfile", "-File", $PSCommandPath, "-CiPath", $probeCiPath, "-ReleasePath", $ReleasePath, "-P0Path", $probeP0Path, "-PackagePath", $probePackagePath, "-DisplayCoordinatorPath", $probeDisplayCoordinatorPath)) {
        $startInfo.ArgumentList.Add($argument)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $started = $false
    $processCompleted = $false
    try {
        # All exact fixture paths are owned before any creation, even on a partial write.
        [System.IO.File]::WriteAllText($probeCiPath, $probeCi, [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText($probeP0Path, $probeP0, [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText($probePackagePath, $probePackage, [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText($probeDisplayCoordinatorPath, $probeDisplayCoordinator, [System.Text.UTF8Encoding]::new($false))
        if (-not $process.Start()) { throw "Workflow regression probe could not start its validator." }
        $started = $true
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(120000)) {
            $process.Kill($true)
            $process.WaitForExit()
            $processCompleted = $true
            throw "Workflow regression probe timed out."
        }
        $processCompleted = $true
        [void]$stdout.GetAwaiter().GetResult()
        [void]$stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -eq 0) {
            throw "Workflow regression probe '$RegressionProbe' accepted an invalid workflow."
        }
    }
    finally {
        if ($started -and -not $processCompleted) {
            try {
                $process.Kill($true)
                $process.WaitForExit()
            }
            catch {}
        }
        $process.Dispose()
        foreach ($probePath in @($probeCiPath, $probeP0Path, $probePackagePath, $probeDisplayCoordinatorPath)) {
            if (Test-Path -LiteralPath $probePath -PathType Leaf) {
                [System.IO.File]::Delete($probePath)
            }
            if (Test-Path -LiteralPath $probePath) {
                throw "Workflow regression probe cleanup failed."
            }
        }
    }
    exit 0
}

$failures = [System.Collections.Generic.List[string]]::new()

foreach ($workflow in @(
        [pscustomobject]@{ Name = "CI"; Text = $ci },
        [pscustomobject]@{ Name = "Release"; Text = $release }
    )) {
    Assert-Absent -Text $workflow.Text -Pattern "continue-on-error" -Label "$($workflow.Name) continue-on-error" -Failures $failures
    foreach ($legacy in @("libssh2", "openssl", "vcpkg", "wezterm-ssh", "05343b")) {
        Assert-Absent -Text $workflow.Text -Pattern $legacy -Label "$($workflow.Name) legacy dependency '$legacy'" -Failures $failures
    }
    Assert-Exactly -Text $workflow.Text -Pattern "(?ms)^defaults:\s*\r?\n\s*run:\s*\r?\n\s*shell:\s*pwsh\s*$" -Expected 1 -Label "$($workflow.Name) PowerShell default shell" -Failures $failures
    Assert-OnlyPowerShellShells -Text $workflow.Text -Label "$($workflow.Name) shell" -Failures $failures
    Assert-Absent -Text $workflow.Text -Pattern "(?i)wezterm" -Label "$($workflow.Name) WezTerm terminal runtime" -Failures $failures
}

Assert-Contains -Text $ci -Pattern "(?ms)^permissions:\s*\r?\n\s*contents:\s*read\s*$" -Label "CI least-privilege permissions" -Failures $failures
foreach ($runner in @(
        "(?ms)^\s*- name: Linux x86_64\s*\r?\n\s*os: ubuntu-24\.04\s*$",
        "(?ms)^\s*- name: macOS arm64\s*\r?\n\s*os: macos-26\s*$",
        "(?ms)^\s*- name: Windows x86_64\s*\r?\n\s*os: windows-2022\s*$"
    )) {
    Assert-Exactly -Text $ci -Pattern $runner -Expected 1 -Label "CI runner matrix entry '$runner'" -Failures $failures
}

Assert-Absent -Text $ci -Pattern "(?im)^ {8}if:\s*false\s*(?:#.*)?$" -Label "CI disabled step" -Failures $failures
$failureCheck = 'if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }'
$workspaceStep = Assert-NamedStep -Text $ci -Name "Run required workspace gates" -Failures $failures
Assert-StepHasNoYamlCondition -Step $workspaceStep -Name "Run required workspace gates" -Failures $failures
$displaySetupName = "Prepare workspace display (Windows)"
$displayRestoreName = "Restore workspace display (Windows)"
$displaySetup = Assert-NamedStep -Text $ci -Name $displaySetupName -Failures $failures
$displayRestore = Assert-NamedStep -Text $ci -Name $displayRestoreName -Failures $failures
foreach ($pattern in @(
        "(?m)^ {8}if: runner\.os == 'Windows'\s*$",
        '(?m)^ {10}\$ErrorActionPreference = ''Stop''\s*$',
        '(?m)^ {10}\. scripts/qa/windows-display-experiment\.ps1\s*$',
        '(?m)^ {12}Invoke-WorkspaceDisplaySetup -RunnerTemp \$env:RUNNER_TEMP -Width 1920 -Height 1080\s*$',
        "(?m)^ {10}catch \{ \[Console\]::Error\.WriteLine\('Windows workspace display setup failed; owned state retained if required\.'\); exit 1 \}\s*$"
    )) {
    Assert-StepPattern -Step $displaySetup -Pattern $pattern -Name $displaySetupName -Failures $failures
}
foreach ($pattern in @(
        "(?m)^ {8}if: always\(\) && runner\.os == 'Windows'\s*$",
        '(?m)^ {10}\$ErrorActionPreference = ''Stop''\s*$',
        '(?m)^ {10}\. scripts/qa/windows-display-experiment\.ps1\s*$',
        '\$displayRoot = \$env:RSHELL_WORKSPACE_DISPLAY_ROOT',
        '(?m)^ {10}\$displayLedger = if \(\[string\]::IsNullOrWhiteSpace\(\$displayRoot\)\) \{ '''' \} else \{ Join-Path \$displayRoot ''display-mode\.json'' \}\s*$',
        '(?m)^ {12}Invoke-WorkspaceDisplayRestore -Root \$displayRoot -RunnerTemp \$env:RUNNER_TEMP -Ledger \$displayLedger -Started \$env:RSHELL_WORKSPACE_DISPLAY_STARTED\s*$',
        "(?m)^ {10}catch \{ \[Console\]::Error\.WriteLine\('Windows workspace display restoration failed; owned state retained\.'\); exit 1 \}\s*$"
    )) {
    Assert-StepPattern -Step $displayRestore -Pattern $pattern -Name $displayRestoreName -Failures $failures
}
Assert-Absent -Text ([string]$displayRestore) -Pattern 'exit 0|\breturn\b|Test-Path|apply-started' -Label 'Workflow bypass of display restoration handoff' -Failures $failures
Assert-Exactly -Text $ci -Pattern '(?m)^\s*Invoke-WorkspaceDisplaySetup\b' -Expected 1 -Label 'Single Windows display setup' -Failures $failures
Assert-Exactly -Text $ci -Pattern '(?m)^\s*Invoke-WorkspaceDisplayRestore\b' -Expected 1 -Label 'Single Windows display restoration' -Failures $failures
$displaySetupMatches = @(Get-NamedStepBlock -Text $ci -Name $displaySetupName)
$displayRestoreMatches = @(Get-NamedStepBlock -Text $ci -Name $displayRestoreName)
$workspaceMatches = @(Get-NamedStepBlock -Text $ci -Name "Run required workspace gates")
$windowsP0Matches = @(Get-NamedStepBlock -Text $ci -Name "Run Credential Manager vault probe and P0 All smoke (Windows)")
if ($displaySetupMatches.Count -eq 1 -and $displayRestoreMatches.Count -eq 1 -and $workspaceMatches.Count -eq 1 -and $windowsP0Matches.Count -eq 1 -and
    -not ($displaySetupMatches[0].Index -lt $workspaceMatches[0].Index -and $workspaceMatches[0].Index -lt $windowsP0Matches[0].Index -and $windowsP0Matches[0].Index -lt $displayRestoreMatches[0].Index)) {
    Add-ContractFailure -Failures $failures -Message "One Windows display lifecycle must span workspace, terminal-engine and P0 before always restoration."
}
Assert-Absent -Text ($displaySetup + $displayRestore) -Pattern 'RSHELL_WORKSPACE_DISPLAY_APPLY_STARTED' -Label 'Duplicate display obligation state' -Failures $failures
Assert-Absent -Text $ci -Pattern 'windows-display\.ps1|Set-WorkspaceWorkArea|Set-WorkspaceDisplay|Write-DisplayNewFile|Read-Display(?:WorkArea)?Baseline' -Label 'Independent display mutation or baseline in CI' -Failures $failures
Assert-StepPattern -Step $workspaceStep -Pattern '(?m)^ {10}if \(''\$\{\{ runner\.os \}\}'' -eq ''Windows''\) \{' -Name 'Windows-only workspace display observations' -Failures $failures
Assert-ReadOnlyDisplayObservation -Step $workspaceStep -Phase 'before_workspace' -Gate 'cargo fmt --all -- --check' -Name 'Run required workspace gates' -Failures $failures
foreach ($gate in @(
        "cargo fmt --all -- --check",
        "cargo check --workspace --all-targets --all-features --locked",
        "cargo test --workspace --all-features --locked",
        "cargo test --locked --test production_module_limits",
        "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings"
    )) {
    Assert-StepLine -Step $workspaceStep -Line $gate -Name "Run required workspace gates" -Failures $failures
}
Assert-StepLineCount -Step $workspaceStep -Line $failureCheck -Expected 4 -Name "Run required workspace gates" -Failures $failures
Assert-StepLine -Step $workspaceStep -Line 'if ($workspaceTestExitCode -ne 0) { exit $workspaceTestExitCode }' -Name "Run required workspace gates" -Failures $failures
foreach ($pattern in @(
        "\$env:DISPLAY = ':98'", 'Start-Process -FilePath Xvfb',
        'Stop-Process -Id \$displayServer\.Id -Force'
    )) {
    Assert-StepPattern -Step $workspaceStep -Pattern $pattern -Name "Run required workspace gates" -Failures $failures
}

$ciTerminalGate = Assert-TerminalEngineGateStep -Text $ci -Workflow "CI" -FailureCheck $failureCheck -Failures $failures
$workspaceGateMatches = @(Get-NamedStepBlock -Text $ci -Name "Run required workspace gates")
if ($null -ne $ciTerminalGate -and $workspaceGateMatches.Count -eq 1 -and $ciTerminalGate.Index -le $workspaceGateMatches[0].Index) {
    Add-ContractFailure -Failures $failures -Message "CI terminal-engine gate must run after required workspace gates."
}

$openSshToolsStep = Assert-NamedStep -Text $ci -Name "Confirm system OpenSSH tools" -Failures $failures
Assert-StepHasNoYamlCondition -Step $openSshToolsStep -Name "Confirm system OpenSSH tools" -Failures $failures
Assert-StepLine -Step $openSshToolsStep -Line '$ssh = Get-Command ssh -ErrorAction Stop' -Name "Confirm system OpenSSH tools" -Failures $failures
Assert-StepLine -Step $openSshToolsStep -Line '$sshKeygen = Get-Command ssh-keygen -ErrorAction Stop' -Name "Confirm system OpenSSH tools" -Failures $failures

Assert-Absent -Text $ci -Pattern 'Run bounded SSH surface smoke' -Label "CI duplicate SSH smoke step" -Failures $failures
Assert-Exactly -Text $ci -Pattern 'p0-smoke\.ps1 -Mode All' -Expected 2 -Label "CI Linux/Windows All smoke" -Failures $failures
Assert-Exactly -Text $ci -Pattern 'p0-smoke\.ps1 -Mode Ssh' -Expected 1 -Label "CI macOS Ssh smoke" -Failures $failures
Assert-Absent -Text $ci -Pattern 'cargo test --locked -p rshell-session --test ssh_smoke system_openssh_agent_authenticates_against_local_server' -Label "CI unbounded system-agent smoke" -Failures $failures

foreach ($modeAllStep in @(
        [pscustomobject]@{ Name = "Run Secret Service vault probe and P0 All smoke (Linux)"; Condition = "runner.os == 'Linux'"; Mode = "All" },
        [pscustomobject]@{ Name = "Run temporary keychain vault probe and P0 Ssh smoke (macOS)"; Condition = "runner.os == 'macOS'"; Mode = "Ssh" },
        [pscustomobject]@{ Name = "Run Credential Manager vault probe and P0 All smoke (Windows)"; Condition = "runner.os == 'Windows'"; Mode = "All" }
    )) {
    $step = Assert-NamedStep -Text $ci -Name $modeAllStep.Name -Failures $failures
    if ($null -eq $step -or -not [regex]::IsMatch($step, "(?m)^ {8}if:\s*$([regex]::Escape($modeAllStep.Condition))\s*$")) {
        Add-ContractFailure -Failures $failures -Message "CI step '$($modeAllStep.Name)' must have its exact platform condition."
    }
    if ($null -eq $step -or [regex]::Matches($step, "(?m)^\s*pwsh -NoProfile -File scripts/qa/p0-smoke\.ps1 -Mode $($modeAllStep.Mode)\s*$").Count -ne 1) {
        Add-ContractFailure -Failures $failures -Message "CI step '$($modeAllStep.Name)' must run exactly one P0 $($modeAllStep.Mode) smoke command."
    }
    Assert-UnconditionalSmokeCommand -Step $step -Name $modeAllStep.Name -Failures $failures
    $forbiddenMode = if ($modeAllStep.Mode -eq 'All') { 'Ssh' } else { 'All' }
    Assert-Absent -Text ([string]$step) -Pattern "p0-smoke\.ps1 -Mode $forbiddenMode" -Label "$($modeAllStep.Name) forbidden mode" -Failures $failures
    Assert-StepPattern -Step $step -Name $modeAllStep.Name -Failures $failures -Pattern '(?ms)^\s*try \{\r?\n\s*cargo test --locked -p rshell-storage --features test-support --test system_vault system_vault_real_os_probe_uses_coordinator_and_cleans_random_entry -- --ignored --exact --nocapture\r?\n\s*if \(\$LASTEXITCODE -ne 0\) \{ throw "system vault probe failed" \}\r?\n\s*\}\r?\n\s*finally \{\r?\n\s*cargo test --locked -p rshell-storage --features test-support --test system_vault system_vault_cleanup_exact_parent_reference -- --ignored --exact --nocapture\r?\n\s*if \(\$LASTEXITCODE -ne 0\) \{ throw "system vault cleanup failed" \}'
    $modeAllMatches = @(Get-NamedStepBlock -Text $ci -Name $modeAllStep.Name)
    if ($null -ne $ciTerminalGate -and $modeAllMatches.Count -eq 1 -and $ciTerminalGate.Index -ge $modeAllMatches[0].Index) {
        Add-ContractFailure -Failures $failures -Message "CI terminal-engine gate must run before '$($modeAllStep.Name)'."
    }
}
Assert-Exactly -Text $ci -Pattern "(?m)system_vault_real_os_probe_uses_coordinator_and_cleans_random_entry" -Expected 3 -Label "CI ignored system vault probe" -Failures $failures
Assert-Exactly -Text $ci -Pattern "(?m)system_vault_cleanup_exact_parent_reference" -Expected 3 -Label "CI ignored system vault exact cleanup" -Failures $failures

$windowsAgentStart = Assert-NamedStep -Text $ci -Name "Start Credential Manager SSH agent (Windows)" -Failures $failures
foreach ($pattern in @(
        "(?m)^ {8}if:\s*runner\.os == 'Windows'\s*$", "RSHELL_WINDOWS_SSH_AGENT_BASELINE_STATUS",
        "RSHELL_WINDOWS_SSH_AGENT_BASELINE_START_MODE", "Set-Service -Name ssh-agent -StartupType Manual"
    )) {
    Assert-StepPattern -Step $windowsAgentStart -Pattern $pattern -Name "Start Credential Manager SSH agent (Windows)" -Failures $failures
}
$windowsAgentStop = Assert-NamedStep -Text $ci -Name "Stop Credential Manager SSH agent (Windows)" -Failures $failures
foreach ($pattern in @(
        "(?m)^ {8}if:\s*always\(\) && runner\.os == 'Windows'\s*$", '\$cleanupErrors', "status restoration failed",
        "startup-type restoration failed", 'Set-Service -Name ssh-agent -StartupType \$startupType', "startup-type verification failed",
        '\$missingBaselineStatus -and \$missingBaselineStartupMode', '\$missingBaselineStatus -xor \$missingBaselineStartupMode'
    )) {
    Assert-StepPattern -Step $windowsAgentStop -Pattern $pattern -Name "Stop Credential Manager SSH agent (Windows)" -Failures $failures
}
$macosModeAll = Assert-NamedStep -Text $ci -Name "Run temporary keychain vault probe and P0 Ssh smoke (macOS)" -Failures $failures
foreach ($pattern in @(
        "security list-keychains", '\$cleanupErrors', "default keychain restore failed", "temporary keychain delete failed",
        "vault root cleanup failed", "P0_NATIVE_GUI_SKIP platform=macos", "keychain search list restore failed"
    )) {
    Assert-StepPattern -Step $macosModeAll -Pattern $pattern -Name "Run temporary keychain vault probe and P0 Ssh smoke (macOS)" -Failures $failures
}
$windowsModeAll = Assert-NamedStep -Text $ci -Name "Run Credential Manager vault probe and P0 All smoke (Windows)" -Failures $failures
Assert-ReadOnlyDisplayObservation -Step $windowsModeAll -Phase 'before_p0' -Gate 'pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All' -Name 'Windows P0' -Failures $failures
Assert-Absent -Text ([string]$windowsModeAll) -Pattern '\$display(?:Root|Ledger)|RSHELL_WORKSPACE_DISPLAY_ROOT|workarea\.json|apply-started' -Label 'Display recovery state inside P0 vault lifecycle' -Failures $failures
Assert-StepPattern -Step $windowsModeAll -Pattern '(?ms)^ {10}\$vaultRoot = Join-Path \$env:RUNNER_TEMP "rshell-windows-p0-.*?^ {10}finally \{\r?\n {12}if \(Test-Path -LiteralPath \$vaultRoot -PathType Container\) \{\r?\n {14}Remove-Item -LiteralPath \$vaultRoot -Recurse -Force\r?\n {12}\}\r?\n {10}\}' -Name 'Independent exact Windows P0 vault-root cleanup' -Failures $failures
$displayHelper = Get-Content -LiteralPath (Join-Path $PSScriptRoot "windows-display.ps1") -Raw
$displayNative = Get-Content -LiteralPath (Join-Path $PSScriptRoot "windows-display-native.ps1") -Raw
foreach ($pattern in @('EnumDisplaySettings', 'ChangeDisplaySettings', 'PreferredAtLeast', 'CDS_TEST', 'CDS_FULLSCREEN', 'The display mode did not converge\.')) {
    Assert-Contains -Text $displayNative -Pattern $pattern -Label "Windows display native helper '$pattern'" -Failures $failures
}
foreach ($pattern in @('\$restoreMode = \[RshellDisplayMode\]::new\(\)', '\[string\]\$ChangeKind = "Fullscreen"', '(?s)"Restore" \{.*?\$ChangeKind = "Dynamic"', '\$actual = Get-WorkspaceDisplayCurrent')) {
    Assert-Contains -Text $displayHelper -Pattern $pattern -Label "Windows display helper '$pattern'" -Failures $failures
}
# These source-wiring assertions do not prove hosted native geometry or recovery.
foreach ($pattern in @(
        '(?s)Publish-WorkspaceDisplayRoot \$displayRoot.*?\$baseline = Get-WorkspaceDisplayCurrent.*?\$workAreaBaseline = Get-WorkspaceDisplayWorkArea.*?Write-DisplayNewFile \$displayLedger.*?Write-DisplayNewFile \(Join-Path \$displayRoot ''workarea.json''\) \(\$workAreaBaseline \| ConvertTo-Json -Compress\).*?Assert-DisplayEqual \(Read-DisplayBaseline \$displayLedger\) \$baseline.*?Assert-DisplayRectEqual \(Read-DisplayWorkAreaBaseline \(Join-Path \$displayRoot ''workarea.json''\)\) \$workAreaBaseline.*?\$target = Select-WorkspaceDisplayTarget.*?Write-DisplayNewFile \(Join-Path \$displayRoot ''apply-started''\) \$started.*?Publish-WorkspaceDisplayStarted \$displayRoot.*?Invoke-WorkspaceDisplayChild',
        'GetDirectoryName\(\$ownedRoot\) -ne \$runnerPath', 'rshell-workspace-display-\[0-9a-f\]\{32\}',
        "@\('fullscreen', 'dynamic', 'final'\)", 'Reset-WorkspaceDisplayBaseline \$displayRoot \$displayLedger \$baseline \$workAreaBaseline ''between''',
        '(?s)function Invoke-WorkspaceDisplayRestore.*?\$baseline = Read-DisplayBaseline \$Ledger.*?\$workAreaBaseline = Read-DisplayWorkAreaBaseline.*?Assert-WorkspaceDisplayReady.*?Restore-WorkspaceDisplayBaselines \$ownedRoot \$Ledger \$baseline \$workAreaBaseline ''always'' \$Started.*?Clear-WorkspaceDisplayRestored',
        '(?s)function Reset-WorkspaceDisplayBaseline.*?''Restore'' ''Dynamic''.*?\$current = Get-WorkspaceDisplayCurrent.*?Assert-DisplayEqual \$current \$Baseline',
        '(?m)^ {8}Write-DisplayRectObservation ''baseline'' ''setup'' \$workAreaBaseline\s*$',
        '(?m)^ {8}\$child = Invoke-WorkspaceDisplayChild \$Root \(Join-Path \$Root ''workarea.json''\) ''Restore'' ''Dynamic'' \$WorkAreaBaseline -Operation Workarea\s*$',
        '(?s)\$current = Get-WorkspaceDisplayWorkArea\r?\n\s*Write-DisplayRectObservation ''restore_after_exit'' \$Arm \$current\r?\n\s*Assert-DisplayRectEqual \$current \$WorkAreaBaseline',
        'RSHELL_WORKSPACE_DISPLAY_STARTED=0', 'RSHELL_WORKSPACE_DISPLAY_STARTED=\$\(\[System.IO.Path\]::GetFileName\(\$Root\)\)',
        '\$Started -cne ''0'' -and \$Started -cne \[System.IO.Path\]::GetFileName\(\$ownedRoot\)',
        'Started display root is missing; restoration unverified\.',
        'child-exit-unconfirmed', 'Display mode did not match all four fields\.'
    )) {
    Assert-Contains -Text $displayCoordinator -Pattern $pattern -Label "Windows display coordinator '$pattern'" -Failures $failures
}

foreach ($required in @(
        "libgtk-4-dev", "xvfb", "dbus-x11", "gnome-keyring", "dbus-run-session", "gnome-keyring-daemon",
        "brew install gtk4", "security create-keychain", "security unlock-keychain", "security default-keychain", "security delete-keychain",
        "gvsbuild", "Credential Manager", "cmdkey\.exe", "ssh-agent -k", "Stop-Service.*ssh-agent", "Get-Command ssh", "Get-Command ssh-keygen"
    )) {
    Assert-Contains -Text $ci -Pattern $required -Label "CI real-service setup '$required'" -Failures $failures
}
Assert-Contains -Text $ci -Pattern "(?m)^\s*finally\s*\{" -Label "CI cleanup finally block" -Failures $failures

foreach ($workflow in @(
        [pscustomobject]@{ Name = "CI"; Text = $ci },
        [pscustomobject]@{ Name = "Release"; Text = $release }
    )) {
    $gvsbuild = Assert-NamedStep -Text $workflow.Text -Name "Build GTK4 via gvsbuild" -Failures $failures
    foreach ($pattern in @(
            '\$gvsbuildAttempts = 3',
            'for \(\$attempt = 1; \$attempt -le \$gvsbuildAttempts; \$attempt\+\+\)',
            'gvsbuild GTK build failed after 3 attempts'
        )) {
        Assert-StepPattern -Step $gvsbuild -Pattern $pattern -Name "$($workflow.Name) Build GTK4 via gvsbuild" -Failures $failures
    }
}

foreach ($forbidden in @('cargo\.exe', 'pwsh\.exe', 'C:\\gtk-build', 'C:\\Windows\\System32\\OpenSSH', '\$env:TEMP')) {
    Assert-Absent -Text $p0 -Pattern $forbidden -Label "P0 cross-platform hardcoding '$forbidden'" -Failures $failures
}
Assert-Exactly -Text $p0 -Pattern '"\.exe"' -Expected 1 -Label "P0 Windows executable suffix" -Failures $failures
foreach ($required in @(
        '\$platformIsWindows', '\$platformIsLinux', '\$platformIsMacOS', 'Get-Command -Name "cargo" -ErrorAction Stop',
        'Get-Command -Name "pwsh" -ErrorAction Stop', 'Get-Command -Name "ssh" -ErrorAction Stop',
        'Get-Command -Name "ssh-keygen" -ErrorAction Stop',
        'Get-Command -Name "ssh-add" -ErrorAction Stop', 'RSHELL_SHELL', '\[System\.IO\.Path\]::GetTempPath\(\)',
        '\[System\.IO\.Path\]::PathSeparator', 'RSHELL_GTK_ROOT', 'The macOS keychain home is unavailable',
        'RSHELL_P0_SSH_BIN', '-F /dev/null'
    )) {
    Assert-Contains -Text $p0 -Pattern $required -Label "P0 cross-platform requirement '$required'" -Failures $failures
}
Assert-Contains -Text $p0 -Pattern '(?m)^\$baseEnvironment = @\{ G_DEBUG = "fatal-warnings"; RSHELL_SHELL = \$pwsh \}\s*$' -Label "P0 fatal GTK warnings" -Failures $failures
Assert-P0CleanupAndSecretOrder -Text $p0 -Failures $failures

Assert-Contains -Text $release -Pattern "(?ms)^permissions:\s*\r?\n\s*contents:\s*read\s*$" -Label "Release build least-privilege permissions" -Failures $failures
Assert-Absent -Text $release -Pattern "(?im)^ {8}if:\s*false\s*(?:#.*)?$" -Label "Release disabled step" -Failures $failures
Assert-Exactly -Text $release -Pattern "(?ms)^\s*release:\s*\r?\n\s*name: Release\s*\r?\n\s*needs: build\s*\r?\n\s*runs-on: ubuntu-latest\s*\r?\n\s*permissions:\s*\r?\n\s*contents: write\s*$" -Expected 1 -Label "Release publisher scoped write permission" -Failures $failures
foreach ($target in @(
        [pscustomobject]@{ Name = "linux-x86_64"; Os = "ubuntu-24.04"; Target = "x86_64-unknown-linux-gnu" },
        [pscustomobject]@{ Name = "macos-arm64"; Os = "macos-26"; Target = "aarch64-apple-darwin" },
        [pscustomobject]@{ Name = "windows-x86_64"; Os = "windows-2022"; Target = "x86_64-pc-windows-msvc" }
    )) {
    $entry = "(?ms)^\s*- name: $([regex]::Escape($target.Name))\s*\r?\n\s*os: $([regex]::Escape($target.Os))\s*\r?\n\s*target: $([regex]::Escape($target.Target))\s*$"
    Assert-Exactly -Text $release -Pattern $entry -Expected 1 -Label "Release target '$($target.Target)'" -Failures $failures
}
$releaseBuildStep = Assert-NamedStep -Text $release -Name "Build release" -Failures $failures
Assert-StepHasNoYamlCondition -Step $releaseBuildStep -Name "Build release" -Failures $failures
Assert-StepLine -Step $releaseBuildStep -Line 'cargo build --release --workspace --target ${{ matrix.target }} --locked' -Name "Build release" -Failures $failures
Assert-StepLineCount -Step $releaseBuildStep -Line $failureCheck -Expected 1 -Name "Build release" -Failures $failures

Assert-Absent -Text $release -Pattern "Run terminal engine gate|terminal-engine-gate\.ps1" -Label "Release terminal-engine gate" -Failures $failures

$packageProbe = 'pwsh -NoProfile -File scripts/qa/assert-package.ps1 -Target $env:RSHELL_TARGET -Package $env:RSHELL_PACKAGE'
foreach ($packageStep in @(
        [pscustomobject]@{ Name = "Package (Linux/macOS)"; Condition = "runner.os != 'Windows'" },
        [pscustomobject]@{ Name = "Package (Windows)"; Condition = "runner.os == 'Windows'" }
    )) {
    $step = Assert-NamedStep -Text $release -Name $packageStep.Name -Failures $failures
    if ($null -eq $step -or -not [regex]::IsMatch($step, "(?m)^ {8}if:\s*$([regex]::Escape($packageStep.Condition))\s*$")) {
        Add-ContractFailure -Failures $failures -Message "Workflow step '$($packageStep.Name)' must have its exact platform condition."
    }
    Assert-StepLine -Step $step -Line $packageProbe -Name $packageStep.Name -Failures $failures
}
foreach ($runtimeInvocation in @(
        "(?im)^\s*(?:&\s*)?wezterm(?:[-_.][A-Za-z0-9_]+)?(?:\.exe)?(?:\s|$)",
        "(?im)^\s*Start-Process\b[^\r\n]*\bwezterm\b",
        "(?im)^\s*(?:cargo|pwsh)\b[^\r\n]*\bwezterm\b",
        "(?im)^\s*(?:Copy-Item|Compress-Archive|tar)\b[^\r\n]*\bwezterm\b"
    )) {
    Assert-Absent -Text $package -Pattern $runtimeInvocation -Label "Package active WezTerm terminal runtime command" -Failures $failures
}
Assert-Contains -Text $package -Pattern "(?i)wezterm" -Label "Package WezTerm negative QA sentinel" -Failures $failures

$terminalEngine = Read-WorkflowText -Path (Join-Path $PSScriptRoot "terminal-engine-gate.ps1") -Label "Terminal-engine gate"
$terminalRecord = Read-WorkflowText -Path (Join-Path $PSScriptRoot "..\..\crates\rshell-session\TERMINAL_ENGINE.md") -Label "Terminal-engine decision record"
Assert-Contains -Text $terminalEngine -Pattern '(?m)^\$Backend = "alacritty-terminal@0\.26\.0"\s*$' -Label "Terminal-engine Alacritty 0.26 backend" -Failures $failures
Assert-Contains -Text $terminalRecord -Pattern '(?m)^Decision: \*\*GO\*\*\s*$' -Label "Terminal-engine recorded GO decision" -Failures $failures
Assert-Contains -Text $terminalRecord -Pattern '(?m)^- Selected sole adapter: `alacritty-terminal@0\.26\.0`\s*$' -Label "Terminal-engine recorded Alacritty 0.26 backend" -Failures $failures
Assert-Absent -Text $terminalEngine -Pattern '(?i)wezterm' -Label "Terminal-engine WezTerm runtime" -Failures $failures
Assert-Absent -Text $terminalRecord -Pattern '(?i)wezterm' -Label "Terminal-engine record WezTerm runtime" -Failures $failures
$packageStartupReport = [regex]::Match($package, '(?ms)^function Assert-StartupReport \{.*?^}\r?$').Value
if ([string]::IsNullOrWhiteSpace($packageStartupReport)) {
    Add-ContractFailure -Failures $failures -Message "Package startup report assertion is missing."
}
foreach ($marker in @(
        "embedded_css_loaded", "embedded_icons_renderable", "embedded_icon_backend",
        "measured_terminal_geometry_ready", "scale_aware_icons_ready", "icon_backend", "icon_count", "adaptive_layout_modes",
        "Assert-NoProductAssetPayload", "external-icon-payload", "runtime-icon-backends",
        'Get-Command -Name "pwsh" -ErrorAction Stop', '\$startInfo\.Environment\["RSHELL_SHELL"\] = \$pwsh\.Source',
        '\$startupAttempts = 2', 'if \(\$timedOut -and \$attempt -lt \$startupAttempts\)'
    )) {
    $contractText = if ($marker -in @(
            "measured_terminal_geometry_ready", "scale_aware_icons_ready", "icon_backend", "icon_count", "adaptive_layout_modes"
        )) { $packageStartupReport } else { $package }
    Assert-Contains -Text $contractText -Pattern $marker -Label "Package embedded-resource contract '$marker'" -Failures $failures
}
Assert-Absent -Text $release -Pattern "(?im)(Copy-Item|\bcp\b|Compress-Archive|\btar\b).*?(resources([\\/]icons)?|icons|\*\.svg)" -Label "Release external product icon payload" -Failures $failures
foreach ($required in @(
        "Copy-Item.*LICENSE", "Copy-Item.*README\.md", "gdk-pixbuf-query-loaders\.exe", "gschemas\.compiled", "etc\\fonts",
        "stagedQueryLoaders", "loader cache is not relocatable", "startsWith\(github\.ref, 'refs/tags/'\)",
        "softprops/action-gh-release@v2", "Update Nightly", "actions/upload-artifact@v4"
    )) {
    Assert-Contains -Text $release -Pattern $required -Label "Release packaging/release requirement '$required'" -Failures $failures
}

if ($failures.Count -gt 0) {
    foreach ($failure in $failures) {
        [Console]::Error.WriteLine("workflow-contract: $failure")
    }
    exit 1
}

exit 0
