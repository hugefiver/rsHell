param(
    [string]$CiPath = (Join-Path $PSScriptRoot "..\..\.github\workflows\ci.yml"),
    [string]$ReleasePath = (Join-Path $PSScriptRoot "..\..\.github\workflows\release.yml"),
    [string]$P0Path = (Join-Path $PSScriptRoot "p0-smoke.ps1"),
    [string]$PackagePath = (Join-Path $PSScriptRoot "assert-package.ps1"),
    [AllowEmptyString()][string]$CiText = "",
    [AllowEmptyString()][string]$ReleaseText = "",
    [AllowEmptyString()][string]$P0Text = "",
    [AllowEmptyString()][string]$PackageText = "",
    [ValidateSet("", "dead-workspace-gate", "dead-terminal-engine-gate", "conditional-terminal-engine-gate", "continue-terminal-engine-gate", "missing-terminal-engine-gate", "duplicate-terminal-engine-gate", "misplaced-terminal-engine-gate", "skipped-p0-gate", "conditional-p0-gate", "continued-p0-gate", "missing-fatal-gtk-warnings", "missing-package-startup-field", "missing-platform-matrix-member", "weakened-cleanup-secret-ordering")]
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

if ($RegressionProbe.Length -gt 0) {
    $probeCi = $ci
    $probeP0 = $p0
    $probePackage = $package
    switch ($RegressionProbe) {
        "dead-workspace-gate" {
            $stepHeader = "      - name: Run required workspace gates"
            $probeCi = $ci.Replace($stepHeader, "$stepHeader`n        if: false")
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
    if ($probeCi -ceq $ci -and $probeP0 -ceq $p0 -and $probePackage -ceq $package) {
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
    [System.IO.File]::WriteAllText($probeCiPath, $probeCi, [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($probeP0Path, $probeP0, [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($probePackagePath, $probePackage, [System.Text.UTF8Encoding]::new($false))
    $pwsh = (Get-Command -Name "pwsh" -ErrorAction Stop).Source
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $pwsh
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in @("-NoProfile", "-File", $PSCommandPath, "-CiPath", $probeCiPath, "-ReleasePath", $ReleasePath, "-P0Path", $probeP0Path, "-PackagePath", $probePackagePath)) {
        $startInfo.ArgumentList.Add($argument)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $started = $false
    $processCompleted = $false
    try {
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
        foreach ($probePath in @($probeCiPath, $probeP0Path, $probePackagePath)) {
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
Assert-Absent -Text $ci -Pattern 'p0-smoke\.ps1 -Mode Ssh' -Label "CI duplicate SSH smoke invocation" -Failures $failures
Assert-Absent -Text $ci -Pattern 'cargo test --locked -p rshell-session --test ssh_smoke system_openssh_agent_authenticates_against_local_server' -Label "CI unbounded system-agent smoke" -Failures $failures

foreach ($modeAllStep in @(
        [pscustomobject]@{ Name = "Run Secret Service vault probe and P0 All smoke (Linux)"; Condition = "runner.os == 'Linux'" },
        [pscustomobject]@{ Name = "Run temporary keychain vault probe and P0 All smoke (macOS)"; Condition = "runner.os == 'macOS'" },
        [pscustomobject]@{ Name = "Run Credential Manager vault probe and P0 All smoke (Windows)"; Condition = "runner.os == 'Windows'" }
    )) {
    $step = Assert-NamedStep -Text $ci -Name $modeAllStep.Name -Failures $failures
    if ($null -eq $step -or -not [regex]::IsMatch($step, "(?m)^ {8}if:\s*$([regex]::Escape($modeAllStep.Condition))\s*$")) {
        Add-ContractFailure -Failures $failures -Message "CI step '$($modeAllStep.Name)' must have its exact platform condition."
    }
    if ($null -eq $step -or [regex]::Matches($step, "(?m)^\s*pwsh -NoProfile -File scripts/qa/p0-smoke\.ps1 -Mode All\s*$").Count -ne 1) {
        Add-ContractFailure -Failures $failures -Message "CI step '$($modeAllStep.Name)' must run exactly one P0 All smoke command."
    }
    $modeAllMatches = @(Get-NamedStepBlock -Text $ci -Name $modeAllStep.Name)
    if ($null -ne $ciTerminalGate -and $modeAllMatches.Count -eq 1 -and $ciTerminalGate.Index -ge $modeAllMatches[0].Index) {
        Add-ContractFailure -Failures $failures -Message "CI terminal-engine gate must run before '$($modeAllStep.Name)'."
    }
}
Assert-Exactly -Text $ci -Pattern "(?m)system_vault_real_os_probe_uses_coordinator_and_cleans_random_entry" -Expected 3 -Label "CI ignored system vault probe" -Failures $failures

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
$macosModeAll = Assert-NamedStep -Text $ci -Name "Run temporary keychain vault probe and P0 All smoke (macOS)" -Failures $failures
foreach ($pattern in @(
        "security list-keychains", '\$cleanupErrors', "default keychain restore failed", "temporary keychain delete failed",
        "vault root cleanup failed"
    )) {
    Assert-StepPattern -Step $macosModeAll -Pattern $pattern -Name "Run temporary keychain vault probe and P0 All smoke (macOS)" -Failures $failures
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
