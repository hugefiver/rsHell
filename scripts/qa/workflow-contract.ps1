param(
    [string]$CiPath = (Join-Path $PSScriptRoot "..\..\.github\workflows\ci.yml"),
    [string]$ReleasePath = (Join-Path $PSScriptRoot "..\..\.github\workflows\release.yml"),
    [string]$P0Path = (Join-Path $PSScriptRoot "p0-smoke.ps1"),
    [string]$PackagePath = (Join-Path $PSScriptRoot "assert-package.ps1"),
    [AllowEmptyString()][string]$CiText = "",
    [AllowEmptyString()][string]$ReleaseText = "",
    [AllowEmptyString()][string]$P0Text = "",
    [AllowEmptyString()][string]$PackageText = "",
    [ValidateSet("", "dead-workspace-gate")]
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

$ci = if ($CiText.Length -gt 0) { $CiText } else { Read-WorkflowText -Path $CiPath -Label "CI" }
$release = if ($ReleaseText.Length -gt 0) { $ReleaseText } else { Read-WorkflowText -Path $ReleasePath -Label "Release" }
$p0 = if ($P0Text.Length -gt 0) { $P0Text } else { Read-WorkflowText -Path $P0Path -Label "P0 smoke" }
$package = if ($PackageText.Length -gt 0) { $PackageText } else { Read-WorkflowText -Path $PackagePath -Label "Package assertion" }

if ($RegressionProbe -eq "dead-workspace-gate") {
    $stepHeader = "      - name: Run required workspace gates"
    $deadGateCi = $ci.Replace($stepHeader, "$stepHeader`n        if: false")
    if ($deadGateCi -ceq $ci) {
        throw "Workflow regression probe could not inject a dead workspace gate."
    }
    $temporaryRoot = [System.IO.Path]::GetTempPath()
    if (-not (Test-Path -LiteralPath $temporaryRoot -PathType Container)) {
        throw "Workflow regression probe temporary directory is unavailable."
    }
    $probeCiPath = Join-Path $temporaryRoot "rshell-workflow-contract-$([Guid]::NewGuid().ToString('N')).yml"
    [System.IO.File]::WriteAllText($probeCiPath, $deadGateCi, [System.Text.UTF8Encoding]::new($false))
    $pwsh = (Get-Command -Name "pwsh" -ErrorAction Stop).Source
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $pwsh
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in @("-NoProfile", "-File", $PSCommandPath, "-CiPath", $probeCiPath, "-ReleasePath", $ReleasePath, "-P0Path", $P0Path, "-PackagePath", $PackagePath)) {
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
            throw "Workflow regression probe accepted a disabled workspace gate."
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
        if (Test-Path -LiteralPath $probeCiPath -PathType Leaf) {
            [System.IO.File]::Delete($probeCiPath)
        }
        if (Test-Path -LiteralPath $probeCiPath) {
            throw "Workflow regression probe cleanup failed."
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
}

Assert-Contains -Text $ci -Pattern "(?ms)^permissions:\s*\r?\n\s*contents:\s*read\s*$" -Label "CI least-privilege permissions" -Failures $failures
foreach ($runner in @(
        "(?ms)^\s*- name: Linux x86_64\s*\r?\n\s*os: ubuntu-24\.04\s*$",
        "(?ms)^\s*- name: macOS arm64\s*\r?\n\s*os: macos-15\s*$",
        "(?ms)^\s*- name: Windows x86_64\s*\r?\n\s*os: windows-2022\s*$"
    )) {
    Assert-Exactly -Text $ci -Pattern $runner -Expected 1 -Label "CI runner matrix entry '$runner'" -Failures $failures
}

Assert-Absent -Text $ci -Pattern "(?im)^ {8}if:\s*false\s*(?:#.*)?$" -Label "CI disabled step" -Failures $failures
$failureCheck = 'if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }'
$workspaceStep = Assert-NamedStep -Text $ci -Name "Run required workspace gates" -Failures $failures
Assert-StepHasNoYamlCondition -Step $workspaceStep -Name "Run required workspace gates" -Failures $failures
foreach ($gate in @(
        "cargo fmt -- --check",
        "cargo check --workspace --all-targets --all-features --locked",
        "cargo test --workspace --all-features --locked",
        "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings"
    )) {
    Assert-StepLine -Step $workspaceStep -Line $gate -Name "Run required workspace gates" -Failures $failures
}
Assert-StepLineCount -Step $workspaceStep -Line $failureCheck -Expected 4 -Name "Run required workspace gates" -Failures $failures

$openSshToolsStep = Assert-NamedStep -Text $ci -Name "Confirm system OpenSSH tools" -Failures $failures
Assert-StepHasNoYamlCondition -Step $openSshToolsStep -Name "Confirm system OpenSSH tools" -Failures $failures
Assert-StepLine -Step $openSshToolsStep -Line '$ssh = Get-Command ssh -ErrorAction Stop' -Name "Confirm system OpenSSH tools" -Failures $failures
Assert-StepLine -Step $openSshToolsStep -Line '$sshKeygen = Get-Command ssh-keygen -ErrorAction Stop' -Name "Confirm system OpenSSH tools" -Failures $failures

$nativeSshStep = Assert-NamedStep -Text $ci -Name "Run native SSH smoke" -Failures $failures
Assert-StepHasNoYamlCondition -Step $nativeSshStep -Name "Run native SSH smoke" -Failures $failures
Assert-StepLine -Step $nativeSshStep -Line 'cargo test --locked -p rshell-session --test ssh_smoke -- --nocapture' -Name "Run native SSH smoke" -Failures $failures
Assert-StepLineCount -Step $nativeSshStep -Line $failureCheck -Expected 1 -Name "Run native SSH smoke" -Failures $failures

$systemAgentStep = Assert-NamedStep -Text $ci -Name "Run system OpenSSH agent smoke" -Failures $failures
Assert-StepHasNoYamlCondition -Step $systemAgentStep -Name "Run system OpenSSH agent smoke" -Failures $failures
Assert-StepLine -Step $systemAgentStep -Line 'cargo test --locked -p rshell-session --test ssh_smoke system_openssh_agent_authenticates_against_local_server -- --ignored --exact --nocapture' -Name "Run system OpenSSH agent smoke" -Failures $failures
Assert-StepLineCount -Step $systemAgentStep -Line $failureCheck -Expected 1 -Name "Run system OpenSSH agent smoke" -Failures $failures

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
foreach ($pattern in @("security list-keychains", '\$cleanupErrors', "default keychain restore failed", "temporary keychain delete failed", "vault root cleanup failed")) {
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
        'Get-Command -Name "pwsh" -ErrorAction Stop', 'Get-Command -Name "ssh-keygen" -ErrorAction Stop',
        'Get-Command -Name "ssh-add" -ErrorAction Stop', 'RSHELL_SHELL', '\[System\.IO\.Path\]::GetTempPath\(\)',
        '\[System\.IO\.Path\]::PathSeparator', 'RSHELL_GTK_ROOT'
    )) {
    Assert-Contains -Text $p0 -Pattern $required -Label "P0 cross-platform requirement '$required'" -Failures $failures
}

Assert-Contains -Text $release -Pattern "(?ms)^permissions:\s*\r?\n\s*contents:\s*read\s*$" -Label "Release build least-privilege permissions" -Failures $failures
Assert-Absent -Text $release -Pattern "(?im)^ {8}if:\s*false\s*(?:#.*)?$" -Label "Release disabled step" -Failures $failures
Assert-Exactly -Text $release -Pattern "(?ms)^\s*release:\s*\r?\n\s*name: Release\s*\r?\n\s*needs: build\s*\r?\n\s*runs-on: ubuntu-latest\s*\r?\n\s*permissions:\s*\r?\n\s*contents: write\s*$" -Expected 1 -Label "Release publisher scoped write permission" -Failures $failures
foreach ($target in @(
        [pscustomobject]@{ Name = "linux-x86_64"; Os = "ubuntu-24.04"; Target = "x86_64-unknown-linux-gnu" },
        [pscustomobject]@{ Name = "macos-arm64"; Os = "macos-15"; Target = "aarch64-apple-darwin" },
        [pscustomobject]@{ Name = "windows-x86_64"; Os = "windows-2022"; Target = "x86_64-pc-windows-msvc" }
    )) {
    $entry = "(?ms)^\s*- name: $([regex]::Escape($target.Name))\s*\r?\n\s*os: $([regex]::Escape($target.Os))\s*\r?\n\s*target: $([regex]::Escape($target.Target))\s*$"
    Assert-Exactly -Text $release -Pattern $entry -Expected 1 -Label "Release target '$($target.Target)'" -Failures $failures
}
$releaseBuildStep = Assert-NamedStep -Text $release -Name "Build release" -Failures $failures
Assert-StepHasNoYamlCondition -Step $releaseBuildStep -Name "Build release" -Failures $failures
Assert-StepLine -Step $releaseBuildStep -Line 'cargo build --release --workspace --target ${{ matrix.target }} --locked' -Name "Build release" -Failures $failures
Assert-StepLineCount -Step $releaseBuildStep -Line $failureCheck -Expected 1 -Name "Build release" -Failures $failures

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
foreach ($marker in @(
        "embedded_css_loaded", "embedded_icons_renderable", "embedded_icon_backend",
        "Assert-NoProductAssetPayload", "external-icon-payload", "runtime-icon-backends",
        'Get-Command -Name "pwsh" -ErrorAction Stop', '\$startInfo\.Environment\["RSHELL_SHELL"\] = \$pwsh\.Source'
    )) {
    Assert-Contains -Text $package -Pattern $marker -Label "Package embedded-resource contract '$marker'" -Failures $failures
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
