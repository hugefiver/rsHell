param(
    [ValidateSet("Unit", "Ssh", "Gtk", "Vault", "All")]
    [string]$Mode = "All",
    [AllowEmptyString()][string]$RegressionParserProbe = "",
    [AllowEmptyString()][string]$RegressionCaseProbe = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$platform = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
$platformIsWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)
$platformIsLinux = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Linux)
$platformIsMacOS = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::OSX)
if (-not ($platformIsWindows -or $platformIsLinux -or $platformIsMacOS)) {
    throw "P0 smoke does not support this operating system."
}
$mode = $Mode
$needsUnit = $Mode -in @("Unit", "All")
$needsSsh = $Mode -in @("Ssh", "Gtk", "All")
$needsVault = $Mode -in @("Vault", "Gtk", "All")
$needsGtk = $Mode -in @("Gtk", "All")
$surfaceNames = @(
    "gtk",
    "local_terminal",
    "native_password",
    "native_key",
    "native_keyboard_interactive",
    "system_agent",
    "host_key",
    "vault",
    "imports",
    "tabs_splits",
    "cleanup"
)

function Write-Utf8File {
    param(
        [Parameter(Mandatory)][string]$Path,
        [AllowEmptyString()][string]$Text
    )

    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

function New-ChildStartInfo {
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][AllowEmptyString()][string[]]$Arguments,
        [Parameter(Mandatory)][hashtable]$Environment,
        [Parameter(Mandatory)][string]$WorkingDirectory,
        [bool]$RedirectInput = $false
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.RedirectStandardInput = $RedirectInput
    foreach ($argument in $Arguments) {
        $startInfo.ArgumentList.Add($argument)
    }
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }
    return $startInfo
}

function Start-CapturedChild {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][AllowEmptyString()][string[]]$Arguments,
        [Parameter(Mandatory)][hashtable]$Environment,
        [Parameter(Mandatory)][string]$WorkingDirectory,
        [Parameter(Mandatory)][string]$StdoutPath,
        [Parameter(Mandatory)][string]$StderrPath,
        [AllowNull()][string]$InputText = $null
    )

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = New-ChildStartInfo `
        -FilePath $FilePath `
        -Arguments $Arguments `
        -Environment $Environment `
        -WorkingDirectory $WorkingDirectory `
        -RedirectInput ($null -ne $InputText)
    if (-not $process.Start()) {
        throw "P0 smoke phase '$Name' could not start."
    }
    if ($null -ne $script:ownedChildIds) { $script:ownedChildIds.Add($process.Id) }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if ($null -ne $InputText) {
        $process.StandardInput.Write($InputText)
        $process.StandardInput.Close()
    }
    return [pscustomobject]@{
        Name = $Name
        Process = $process
        StdoutTask = $stdoutTask
        StderrTask = $stderrTask
        StdoutPath = $StdoutPath
        StderrPath = $StderrPath
    }
}

function Complete-CapturedChild {
    param(
        [Parameter(Mandatory)]$Run,
        [Parameter(Mandatory)][int]$TimeoutSeconds,
        [bool]$AllowFailure = $false
    )

    $timedOut = -not $Run.Process.WaitForExit($TimeoutSeconds * 1000)
    if ($timedOut) {
        try { $Run.Process.Kill($true) } catch {}
        $Run.Process.WaitForExit()
    }
    $stdout = $Run.StdoutTask.GetAwaiter().GetResult()
    $stderr = $Run.StderrTask.GetAwaiter().GetResult()
    Write-Utf8File -Path $Run.StdoutPath -Text $stdout
    Write-Utf8File -Path $Run.StderrPath -Text $stderr
    $exitCode = if ($timedOut) { -1 } else { $Run.Process.ExitCode }
    if ($null -ne $script:ownedChildIds) {
        [void]$script:ownedChildIds.Remove($Run.Process.Id)
    }
    $Run.Process.Dispose()
    if ($timedOut) {
        throw "P0 smoke phase '$($Run.Name)' timed out; inspect its redacted artifact logs."
    }
    if (-not $AllowFailure -and $exitCode -ne 0) {
        throw "P0 smoke phase '$($Run.Name)' failed with exit $exitCode; inspect its redacted artifact logs."
    }
    return $exitCode
}

function Invoke-CapturedChild {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][AllowEmptyString()][string[]]$Arguments,
        [Parameter(Mandatory)][hashtable]$Environment,
        [Parameter(Mandatory)][string]$WorkingDirectory,
        [Parameter(Mandatory)][string]$StdoutPath,
        [Parameter(Mandatory)][string]$StderrPath,
        [Parameter(Mandatory)][int]$TimeoutSeconds,
        [AllowNull()][string]$InputText = $null,
        [bool]$AllowFailure = $false
    )

    $run = Start-CapturedChild `
        -Name $Name `
        -FilePath $FilePath `
        -Arguments $Arguments `
        -Environment $Environment `
        -WorkingDirectory $WorkingDirectory `
        -StdoutPath $StdoutPath `
        -StderrPath $StderrPath `
        -InputText $InputText
    return Complete-CapturedChild -Run $run -TimeoutSeconds $TimeoutSeconds -AllowFailure $AllowFailure
}

function Add-Phase {
    param([Parameter(Mandatory)][string]$Name)
    $script:phases.Add([pscustomobject]@{ name = $Name; status = "passed" })
}

function Assert-ExactLibtestListing {
    param(
        [Parameter(Mandatory)][string]$Listing,
        [Parameter(Mandatory)][string]$TestName
    )

    $listed = @([regex]::Matches($Listing, '(?m)^(?<name>.+): test\r?$'))
    $summaries = @([regex]::Matches($Listing, '(?m)^(?<count>\d+) tests?, \d+ benchmarks\r?$'))
    $listedCount = @($summaries | ForEach-Object { [int]$_.Groups['count'].Value } |
            Measure-Object -Sum).Sum
    if ($listed.Count -ne 1 -or $summaries.Count -eq 0 -or $listedCount -ne 1 -or
        $listed[0].Groups['name'].Value -cne $TestName -or
        @($listed | Where-Object { $_.Groups['name'].Value -ceq $TestName }).Count -ne 1) {
        throw "P0 regression exact-test discovery did not yield exactly one matching test."
    }
}

function Assert-ExactLibtestExecution {
    param(
        [Parameter(Mandatory)][string]$Output,
        [Parameter(Mandatory)][string]$TestName,
        [Parameter(Mandatory)][int]$ExitCode
    )

    $escapedName = [regex]::Escape($TestName)
    $running = @([regex]::Matches($Output, '(?m)^running (?<count>\d+) tests?\r?$'))
    $status = @([regex]::Matches($Output, '(?m)^test .+ \.\.\. (?:ok|FAILED|ignored)\r?$'))
    $expectedStatus = @([regex]::Matches($Output, "(?m)^test $escapedName \.\.\. ok\r?`$"))
    $summary = @([regex]::Matches(
            $Output,
            '(?m)^test result: (?<result>ok|FAILED)\. (?<passed>\d+) passed; (?<failed>\d+) failed; (?<ignored>\d+) ignored; (?<measured>\d+) measured; \d+ filtered out; finished in .+\r?$'
        ))
    $runningCount = @($running | ForEach-Object { [int]$_.Groups['count'].Value } |
            Measure-Object -Sum).Sum
    $passedCount = @($summary | ForEach-Object { [int]$_.Groups['passed'].Value } |
            Measure-Object -Sum).Sum
    $failedCount = @($summary | ForEach-Object { [int]$_.Groups['failed'].Value } |
            Measure-Object -Sum).Sum
    $ignoredCount = @($summary | ForEach-Object { [int]$_.Groups['ignored'].Value } |
            Measure-Object -Sum).Sum
    $measuredCount = @($summary | ForEach-Object { [int]$_.Groups['measured'].Value } |
            Measure-Object -Sum).Sum
    if ($ExitCode -ne 0 -or $running.Count -eq 0 -or $summary.Count -eq 0 -or
        $runningCount -ne 1 -or $status.Count -ne 1 -or $expectedStatus.Count -ne 1 -or
        $passedCount -ne 1 -or $failedCount -ne 0 -or $ignoredCount -ne 0 -or $measuredCount -ne 0) {
        throw "P0 regression exact test did not execute and pass exactly once."
    }
}

function Assert-RegressionProbeRejects {
    param([Parameter(Mandatory)][scriptblock]$Operation)

    try {
        & $Operation
    }
    catch {
        return
    }
    throw "P0 regression parser probe accepted invalid libtest output."
}

function Invoke-RegressionParserProbe {
    param([Parameter(Mandatory)][string]$Probe)

    $test = "p0::exact_test"
    $listing = "$test`: test`n`n1 test, 0 benchmarks`n"
    $passing = "running 1 test`ntest $test ... ok`n`ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`n"
    switch ($Probe) {
        "zero" {
            Assert-RegressionProbeRejects {
                Assert-ExactLibtestListing -Listing "`n0 tests, 0 benchmarks`n" -TestName $test
            }
        }
        "one" {
            Assert-ExactLibtestListing -Listing $listing -TestName $test
            Assert-ExactLibtestExecution -Output $passing -TestName $test -ExitCode 0
        }
        "multiple" {
            Assert-RegressionProbeRejects {
                Assert-ExactLibtestListing -Listing "$test`: test`nother::test: test`n`n2 tests, 0 benchmarks`n" -TestName $test
            }
        }
        "failure" {
            Assert-RegressionProbeRejects {
                Assert-ExactLibtestExecution -Output "running 1 test`ntest $test ... FAILED`n`ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`n" -TestName $test -ExitCode 101
            }
        }
        default { throw "P0 regression parser probe is invalid." }
    }
}

function Invoke-RegressionCase {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Package,
        [AllowEmptyString()][string]$Target = "",
        [Parameter(Mandatory)][string[]]$Tests,
        [AllowEmptyString()][string]$Features = ""
    )

    $ordinal = 0
    foreach ($test in $Tests) {
        $ordinal++
        $arguments = [System.Collections.Generic.List[string]]::new()
        foreach ($value in @("test", "--locked", "-p", $Package)) { $arguments.Add($value) }
        if ($Features) {
            $arguments.Add("--features")
            $arguments.Add($Features)
        }
        if ($Target) {
            $arguments.Add("--test")
            $arguments.Add($Target)
        }
        $arguments.Add($test)
        $arguments.Add("--")
        $arguments.Add("--exact")
        $arguments.Add("--list")
        $listingPath = Join-Path $artifactRoot "$stem-regression-$Name-$ordinal.list.stdout.log"
        [void](Invoke-CapturedChild `
                -Name "regression_$Name-$ordinal-list" `
                -FilePath $cargo `
                -Arguments $arguments `
                -Environment $baseEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath $listingPath `
                -StderrPath (Join-Path $artifactRoot "$stem-regression-$Name-$ordinal.list.stderr.log") `
                -TimeoutSeconds 300)
        Assert-ExactLibtestListing -Listing ([System.IO.File]::ReadAllText($listingPath)) -TestName $test

        [void]$arguments.RemoveAt($arguments.Count - 1)
        $resultPath = Join-Path $artifactRoot "$stem-regression-$Name-$ordinal.stdout.log"
        $testExit = Invoke-CapturedChild `
                -Name "regression_$Name-$ordinal" `
                -FilePath $cargo `
            -Arguments $arguments `
            -Environment $baseEnvironment `
            -WorkingDirectory $repoRoot `
            -StdoutPath $resultPath `
            -StderrPath (Join-Path $artifactRoot "$stem-regression-$Name-$ordinal.stderr.log") `
            -TimeoutSeconds 300 `
            -AllowFailure $true
        Assert-ExactLibtestExecution `
            -Output ([System.IO.File]::ReadAllText($resultPath)) `
            -TestName $test `
            -ExitCode $testExit
    }
    Add-Phase "regression_$Name"
}

function Invoke-MainThreadGtkRegression {
    $name = "actor_panic_gtk_survival"
    [void](Invoke-CapturedChild `
            -Name "regression_$name-main-thread" `
            -FilePath $cargo `
            -Arguments @("test", "--locked", "-p", "rshell", "--test", "actor_panic_gtk_survival_macos") `
            -Environment $baseEnvironment `
            -WorkingDirectory $repoRoot `
            -StdoutPath (Join-Path $artifactRoot "$stem-regression-$name-main-thread.stdout.log") `
            -StderrPath (Join-Path $artifactRoot "$stem-regression-$name-main-thread.stderr.log") `
            -TimeoutSeconds 300)
    Add-Phase "regression_$name"
}

function Set-LateFailure {
    param(
        [Parameter(Mandatory)]$Report,
        [Parameter(Mandatory)][string]$Code
    )

    $Report.state = "failed"
    foreach ($surface in @("cleanup", "vault", "system_agent", "native_password", "native_key", "native_keyboard_interactive", "host_key")) {
        $property = $Report.PSObject.Properties[$surface]
        if ($null -eq $property) { continue }
        $property.Value.status = "failed"
        $missing = @($property.Value.missing_evidence)
        if ($missing -notcontains $Code) {
            $property.Value.missing_evidence = @($missing + $Code)
        }
    }
    if (@($script:phases | Where-Object { $_.name -eq "harness_finalization" }).Count -eq 0) {
        $script:phases.Add([pscustomobject]@{
                name = "harness_finalization"
                status = "failed"
                evidence = "late_cleanup_or_security_failure"
            })
    }
}

function Assert-Observation {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Surface,
        [Parameter(Mandatory)][string[]]$Facts,
        [AllowNull()][hashtable]$Binding = $null
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "P0 smoke observation '$Surface' is missing."
    }
    $document = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($document.version -ne 1 -or $document.generated_by -ne "p0_qa" -or
        $document.surface -ne $Surface) {
        throw "P0 smoke observation '$Surface' has an invalid contract."
    }
    foreach ($fact in $Facts) {
        if ($document.observations -notcontains $fact) {
            throw "P0 smoke observation '$Surface' is missing required evidence."
        }
    }
    if ($null -ne $Binding) {
        foreach ($name in @("run_nonce", "fixture", "connection", "endpoint")) {
            if ([string]$document.$name -ne [string]$Binding[$name]) {
                throw "P0 smoke observation '$Surface' has a mismatched binding."
            }
        }
    }
}

function Wait-ForFixtureReady {
    param(
        [Parameter(Mandatory)]$Run,
        [Parameter(Mandatory)][string]$ReadyPath,
        [int]$TimeoutSeconds = 180
    )

    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    while ($timer.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
        if (Test-Path -LiteralPath $ReadyPath -PathType Leaf) {
            return Get-Content -LiteralPath $ReadyPath -Raw | ConvertFrom-Json
        }
        if ($Run.Process.HasExited) {
            [void](Complete-CapturedChild -Run $Run -TimeoutSeconds 1 -AllowFailure $true)
            throw "P0 russh fixture exited before readiness; inspect its redacted artifact logs."
        }
        Start-Sleep -Milliseconds 50
    }
    throw "P0 russh fixture readiness timed out."
}

function Get-AgentIdentitySnapshot {
    param([Parameter(Mandatory)][string]$SshAddPath)

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $SshAddPath
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.ArgumentList.Add("-L")
    $process = [System.Diagnostics.Process]::Start($startInfo)
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(10000)) {
        try { $process.Kill($true) } catch {}
        throw "The real system OpenSSH agent did not answer identity inspection."
    }
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    [void]$stderrTask.GetAwaiter().GetResult()
    $exitCode = $process.ExitCode
    $process.Dispose()
    if ($exitCode -notin @(0, 1)) {
        throw "The real system OpenSSH agent is unavailable."
    }
    return @(($stdout -split "`r?`n") | Where-Object { $_ } | Sort-Object)
}

function Assert-Png {
    param([Parameter(Mandatory)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "The real GTK PNG artifact is missing."
    }
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 24 -or $bytes[0] -ne 137 -or $bytes[1] -ne 80 -or
        $bytes[2] -ne 78 -or $bytes[3] -ne 71) {
        throw "The GTK artifact is not a valid PNG."
    }
    [uint32]$width = ([uint32]$bytes[16] * 16777216) +
        ([uint32]$bytes[17] * 65536) + ([uint32]$bytes[18] * 256) + $bytes[19]
    [uint32]$height = ([uint32]$bytes[20] * 16777216) +
        ([uint32]$bytes[21] * 65536) + ([uint32]$bytes[22] * 256) + $bytes[23]
    if ($width -eq 0 -or $height -eq 0) {
        throw "The GTK PNG has invalid dimensions."
    }
    return [pscustomobject]@{ width = $width; height = $height }
}

function Assert-VisualContract {
    param([Parameter(Mandatory)]$Report, [Parameter(Mandatory)]$PngInfo)

    if ($null -eq $Report.visual -or $null -eq $Report.visual.facts -or $null -eq $Report.visual.png) {
        throw "P0 visual evidence is missing."
    }
    $facts = $Report.visual.facts
    $png = $Report.visual.png
    foreach ($name in @("command_bar", "dense_sidebar", "tab_strip", "pane_command_row", "terminal_canvas", "content_dialog", "focus_or_selection_treatment")) {
        if ($facts.$name -isnot [bool] -or -not $facts.$name) {
            throw "P0 semantic visual fact failed."
        }
    }
    if ($facts.requested_width -ne 1360 -or $facts.requested_height -ne 860) {
        throw "P0 requested viewport changed."
    }
    if ($facts.realized_width -ne $PngInfo.Width -or $facts.realized_height -ne $PngInfo.Height) {
        throw "P0 PNG/report dimensions differ."
    }
    if ($png.width -ne $PngInfo.Width -or $png.height -ne $PngInfo.Height -or -not $png.non_empty) {
        throw "P0 PNG evidence is inconsistent."
    }
    if ($png.dark_regions_required -ne 4 -or $png.dark_regions_passed -ne 4) {
        throw "P0 dark-shell region contract failed."
    }
    if ($png.focus_or_selection_thickness_px -lt 2 -or $png.focus_or_selection_thickness_px -gt 4) {
        throw "P0 focus/selection thickness contract failed."
    }
}

function Write-Junit {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)]$Report,
        [AllowNull()]$Dimensions,
        [AllowNull()][string]$FailureMessage = $null
    )

    $settings = [System.Xml.XmlWriterSettings]::new()
    $settings.Indent = $true
    $settings.Encoding = [System.Text.UTF8Encoding]::new($false)
    $writer = [System.Xml.XmlWriter]::Create($Path, $settings)
    try {
        $writer.WriteStartDocument()
        $writer.WriteStartElement("testsuite")
        $writer.WriteAttributeString("name", "rshell-p0-smoke")
        $includedSurfaces = @($surfaceNames | Where-Object {
                $null -ne $Report.PSObject.Properties[$_]
            })
        $surfaceFailures = @($includedSurfaces | Where-Object {
                $Report.PSObject.Properties[$_].Value.status -ne "passed"
            })
        $phaseFailures = @($phases | Where-Object { $_.status -ne "passed" })
        $hasHarnessFailure = -not [string]::IsNullOrEmpty($FailureMessage)
        $failureCount = $surfaceFailures.Count + $phaseFailures.Count + $(if ($hasHarnessFailure) { 1 } else { 0 })
        $writer.WriteAttributeString("tests", [string]($includedSurfaces.Count + $phases.Count + $(if ($hasHarnessFailure) { 1 } else { 0 })))
        $writer.WriteAttributeString("failures", [string]$failureCount)
        $writer.WriteStartElement("properties")
        foreach ($property in @{
                platform = $platform
                mode = $mode
                png_width = if ($null -eq $Dimensions) { "0" } else { [string]$Dimensions.width }
                png_height = if ($null -eq $Dimensions) { "0" } else { [string]$Dimensions.height }
            }.GetEnumerator()) {
            $writer.WriteStartElement("property")
            $writer.WriteAttributeString("name", $property.Key)
            $writer.WriteAttributeString("value", [string]$property.Value)
            $writer.WriteEndElement()
        }
        $writer.WriteEndElement()
        foreach ($surface in $includedSurfaces) {
            $writer.WriteStartElement("testcase")
            $writer.WriteAttributeString("classname", "p0.surface")
            $writer.WriteAttributeString("name", $surface)
            if ($surfaceFailures -contains $surface) {
                $writer.WriteStartElement("failure")
                $writer.WriteAttributeString("message", "required surface did not pass")
                $writer.WriteEndElement()
            }
            $writer.WriteEndElement()
        }
        foreach ($phase in $phases) {
            $writer.WriteStartElement("testcase")
            $writer.WriteAttributeString("classname", "p0.phase")
            $writer.WriteAttributeString("name", $phase.name)
            if ($phase.status -ne "passed") {
                $writer.WriteStartElement("failure")
                $writer.WriteAttributeString("message", "required phase did not pass")
                $writer.WriteEndElement()
            }
            $writer.WriteEndElement()
        }
        if ($hasHarnessFailure) {
            $writer.WriteStartElement("testcase")
            $writer.WriteAttributeString("classname", "p0.harness")
            $writer.WriteAttributeString("name", "harness_finalization")
            $writer.WriteStartElement("failure")
            $writer.WriteAttributeString("message", "late cleanup or security finalization failed")
            $writer.WriteEndElement()
            $writer.WriteEndElement()
        }
        $writer.WriteEndElement()
        $writer.WriteEndDocument()
    }
    finally {
        $writer.Dispose()
    }
    [xml]$written = Get-Content -LiteralPath $Path -Raw
    $failureNodes = @($written.SelectNodes("//failure"))
    if ([int]$written.testsuite.failures -ne $failureNodes.Count) {
        throw "JUnit failure count does not match its concrete failure nodes."
    }
}

function Add-Action {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[object]]$List,
        [Parameter(Mandatory)][System.Collections.IDictionary]$Action
    )
    if ($script:actionSurface) { $Action["surface"] = $script:actionSurface }
    if ($script:actionConnection) { $Action["connection_label"] = $script:actionConnection }
    $List.Add([pscustomobject]$Action)
}

function Set-ActionBinding {
    param(
        [Parameter(Mandatory)][string]$Surface,
        [AllowNull()][string]$Connection = $null
    )
    $script:actionSurface = $Surface
    $script:actionConnection = $Connection
}

function Add-TextFieldAction {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[object]]$List,
        [Parameter(Mandatory)][string]$Field,
        [Parameter(Mandatory)][string]$Value
    )
    Add-Action $List ([ordered]@{
            action = "set_connection_field"
            field = [ordered]@{ kind = "text"; field = $Field; value = $Value }
        })
}

function Add-ConnectionPrefix {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[object]]$List,
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)]$Endpoint,
        [Parameter(Mandatory)][string]$Transport,
        [Parameter(Mandatory)][string]$Authentication,
        [Parameter(Mandatory)][string]$Surface
    )
    Set-ActionBinding -Surface $Surface -Connection $Surface
    Add-Action $List ([ordered]@{ action = "open_connection_editor" })
    Add-TextFieldAction $List "name" $Name
    Add-TextFieldAction $List "host" ([string]$Endpoint.host)
    Add-TextFieldAction $List "username" "contract-user"
    Add-Action $List ([ordered]@{
            action = "set_connection_field"
            field = [ordered]@{ kind = "port"; port = [int]$Endpoint.port }
        })
    Add-Action $List ([ordered]@{
            action = "set_connection_field"
            field = [ordered]@{ kind = "transport"; transport = $Transport }
        })
    Add-Action $List ([ordered]@{
            action = "set_connection_field"
            field = [ordered]@{ kind = "authentication"; authentication = $Authentication }
        })
}

function Add-SubmitConnect {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.List[object]]$List,
        [Parameter(Mandatory)][string]$Name
    )
    Add-Action $List ([ordered]@{ action = "submit_connection" })
    Add-Action $List ([ordered]@{ action = "select_connection"; connection = $Name })
    Add-Action $List ([ordered]@{ action = "connect" })
}

if ($RegressionParserProbe) {
    Invoke-RegressionParserProbe -Probe $RegressionParserProbe
    exit 0
}

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$temporaryBase = [System.IO.Path]::GetTempPath()
if (-not (Test-Path -LiteralPath $temporaryBase -PathType Container)) {
    throw "P0 smoke temporary directory is unavailable."
}
$pathSeparator = [string][System.IO.Path]::PathSeparator
$binarySuffix = if ($platformIsWindows) { ".exe" } else { "" }
$cargo = (Get-Command -Name "cargo" -ErrorAction Stop).Source
$pwsh = (Get-Command -Name "pwsh" -ErrorAction Stop).Source
$cargoHome = if (-not [string]::IsNullOrWhiteSpace($env:CARGO_HOME)) {
    [System.IO.Path]::GetFullPath($env:CARGO_HOME)
} else {
    [System.IO.Path]::GetFullPath((Split-Path -Parent (Split-Path -Parent $cargo)))
}
$rustupHome = if (-not [string]::IsNullOrWhiteSpace($env:RUSTUP_HOME)) {
    [System.IO.Path]::GetFullPath($env:RUSTUP_HOME)
} else {
    $rustupCommand = Get-Command -Name "rustup" -ErrorAction SilentlyContinue
    if ($rustupCommand) {
        $rustupHomeLines = @(& $rustupCommand.Source show home)
        if ($LASTEXITCODE -ne 0 -or $rustupHomeLines.Count -eq 0) {
            throw "P0 smoke could not resolve the active Rustup home."
        }
        [System.IO.Path]::GetFullPath(($rustupHomeLines -join "`n").Trim())
    } else {
        $null
    }
}
$sshKeygen = $null
$sshAdd = $null
if ($needsSsh) {
    $sshKeygen = (Get-Command -Name "ssh-keygen" -ErrorAction Stop).Source
    $sshAdd = (Get-Command -Name "ssh-add" -ErrorAction Stop).Source
}

$gtkRoot = $null
if ($platformIsWindows) {
    $gtkCandidates = [System.Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($env:RSHELL_GTK_ROOT)) {
        $gtkCandidates.Add($env:RSHELL_GTK_ROOT)
    }
    if (-not [string]::IsNullOrWhiteSpace($env:PKG_CONFIG_PATH)) {
        foreach ($pkgConfigPath in $env:PKG_CONFIG_PATH -split [regex]::Escape($pathSeparator)) {
            if ([string]::IsNullOrWhiteSpace($pkgConfigPath)) { continue }
            $pkgConfigDirectory = [System.IO.Path]::GetFullPath($pkgConfigPath)
            $libDirectory = [System.IO.Path]::GetDirectoryName($pkgConfigDirectory)
            if (([System.IO.Path]::GetFileName($pkgConfigDirectory) -ieq "pkgconfig") -and
                ([System.IO.Path]::GetFileName($libDirectory) -ieq "lib")) {
                $gtkCandidates.Add([System.IO.Path]::GetDirectoryName($libDirectory))
            }
        }
    }
    foreach ($candidate in $gtkCandidates) {
        if ([string]::IsNullOrWhiteSpace($candidate)) { continue }
        $resolvedCandidate = [System.IO.Path]::GetFullPath($candidate)
        if ((Test-Path -LiteralPath (Join-Path $resolvedCandidate "bin") -PathType Container) -and
            (Test-Path -LiteralPath (Join-Path $resolvedCandidate "lib") -PathType Container) -and
            (Test-Path -LiteralPath (Join-Path (Join-Path $resolvedCandidate "lib") "pkgconfig") -PathType Container)) {
            $gtkRoot = $resolvedCandidate
            break
        }
    }
    if ($null -eq $gtkRoot) {
        throw "Windows GTK runtime is unavailable through RSHELL_GTK_ROOT or PKG_CONFIG_PATH."
    }
}

$artifactRoot = Join-Path (Join-Path $repoRoot "artifacts") "p0-smoke"
if (-not (Test-Path -LiteralPath $artifactRoot)) {
    [void](New-Item -ItemType Directory -Path $artifactRoot)
}
$platformSlug = (($platform.ToLowerInvariant() -replace "[^a-z0-9]+", "-").Trim("-"))
$stem = "$platformSlug-$($mode.ToLowerInvariant())"
$artifactReport = Join-Path $artifactRoot "$stem.json"
$artifactPng = Join-Path $artifactRoot "$stem.png"
$artifactJunit = Join-Path $artifactRoot "$stem.junit.xml"
foreach ($stale in @($artifactReport, $artifactPng, $artifactJunit)) {
    if (Test-Path -LiteralPath $stale -PathType Leaf) {
        Remove-Item -LiteralPath $stale -Force
    }
}

$runId = [Guid]::NewGuid().ToString("N")
$tempRoot = Join-Path $temporaryBase "rshell-p0-qa-$runId"
[void](New-Item -ItemType Directory -Path $tempRoot)
$observationRoot = Join-Path $tempRoot "observations"
$fixtureObservationRoot = Join-Path $tempRoot "fixture-observations"
[void](New-Item -ItemType Directory -Path $observationRoot)
[void](New-Item -ItemType Directory -Path $fixtureObservationRoot)
$agentCleanupLedger = Join-Path $tempRoot "agent-cleanup-ledger.json"
$vaultCleanupLedger = Join-Path $tempRoot "vault-cleanup-ledger.json"

$passwordName = "P0_SMOKE_SECRET_PASSWORD"
$passphraseName = "P0_SMOKE_SECRET_KEY_PASSPHRASE"
$kbiVisibleName = "P0_SMOKE_SECRET_KBI_VISIBLE"
$kbiCodeName = "P0_SMOKE_SECRET_KBI_CODE"
$pasteName = "P0_SMOKE_SECRET_PASTE"
$vaultFailureSecretName = "P0_SMOKE_SECRET_VAULT_FAILURE"
$passwordValue = "p0-password-$runId"
$passphraseValue = "p0-passphrase-$runId"
$kbiVisibleValue = "user-visible"
$kbiCodeValue = "one-time-code"
$pasteValue = "Write-Output p0-paste-$runId`r"
$vaultFailureSecretValue = "p0-vault-failure-$runId"
$secretNames = @($passwordName, $passphraseName, $kbiVisibleName, $kbiCodeName, $pasteName, $vaultFailureSecretName)
$secretEnvironment = @{
    $passwordName = $passwordValue
    $passphraseName = $passphraseValue
    $kbiVisibleName = $kbiVisibleValue
    $kbiCodeName = $kbiCodeValue
    $pasteName = $pasteValue
    $vaultFailureSecretName = $vaultFailureSecretValue
}

$baseEnvironment = @{ G_DEBUG = "fatal-warnings"; RSHELL_SHELL = $pwsh }
if ($platformIsLinux) { $baseEnvironment.GTK_A11Y = "none" }
if ($platformIsWindows) {
    $baseEnvironment.PATH = (Join-Path $gtkRoot "bin") + $pathSeparator + $env:PATH
    $baseEnvironment.LIB = Join-Path $gtkRoot "lib"
    $baseEnvironment.PKG_CONFIG_PATH = Join-Path (Join-Path $gtkRoot "lib") "pkgconfig"
}
$childEnvironment = @{}
foreach ($entry in $baseEnvironment.GetEnumerator()) { $childEnvironment[$entry.Key] = $entry.Value }
foreach ($entry in $secretEnvironment.GetEnumerator()) { $childEnvironment[$entry.Key] = $entry.Value }

$phases = [System.Collections.Generic.List[object]]::new()
$script:ownedChildIds = [System.Collections.Generic.List[int]]::new()
$fixtureRun = $null
$fixtureStop = Join-Path $tempRoot "fixture.stop"
$agentBaseline = $null
$agentPrivateKey = Join-Path $tempRoot "parent-agent-key"
$agentPublicKey = "$agentPrivateKey.pub"
$agentCleanupRequired = $false
$vaultReference = "rshell://credential/$runId"
$vaultFailureReference = "rshell://credential/$runId-failure"
$vaultCleanupRequired = $false
$failure = $null
$secretScanRan = $false
$pendingReport = $null
$pendingPngBytes = $null
$pendingDimensions = $null
$script:actionSurface = $null
$script:actionConnection = $null
$ownedProcessesClean = $false

try {
    if ($RegressionCaseProbe) {
        Invoke-RegressionCase `
            -Name "exact_probe" `
            -Package "rshell-ui" `
            -Target "terminal_view_model" `
            -Tests @($RegressionCaseProbe)
        throw "P0 regression probe unexpectedly passed."
    }

    if ($needsUnit) {
        [void](Invoke-CapturedChild `
                -Name "unit-contract" `
                -FilePath $cargo `
                -Arguments @("test", "--locked", "--test", "p0_acceptance") `
                -Environment $baseEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-unit.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-unit.stderr.log") `
                -TimeoutSeconds 240)
        Add-Phase "unit_contract"
    }

    if ($Mode -eq "All") {
        $regressionCases = @(
            @{ Name = "option_like_host"; Package = "rshell-session"; Target = "system_ssh"; Tests = @("option_like_host_nul_and_newline_are_rejected_not_escaped") }
            @{ Name = "ports_0_65536"; Package = "rshell-core"; Target = "connection_catalog"; Tests = @("validation_rejects_invalid_hosts_ports_and_authentication") }
            @{ Name = "resize_1x1_999x999"; Package = "rshell-ui"; Target = "terminal_view_model"; Tests = @("resize_extremes_emit_real_1x1_and_999x999_commands") }
            @{ Name = "wide_midpoint"; Package = "rshell-session"; Target = "engine_contract"; Tests = @("wide_midpoint_selection_normalizes_to_the_stable_wide_cell_and_frame_overlay") }
            @{ Name = "backpressure_10k"; Package = "rshell-session"; Target = "ssh_smoke"; Tests = @("native_backpressure_drains_ten_kib_then_remains_interactive") }
            @{ Name = "unknown_host_reject"; Package = "rshell-session"; Target = "ssh_smoke"; Tests = @("native_rejected_unknown_host_key_fails_closed") }
            @{ Name = "changed_host_key"; Package = "rshell-session"; Target = "ssh_smoke"; Tests = @("native_changed_host_key_fails_closed_without_a_prompt") }
            @{ Name = "wrong_password"; Package = "rshell-session"; Target = "ssh_smoke"; Tests = @("native_wrong_password_is_an_authentication_failure_without_secret_output") }
            @{ Name = "kbi_cancel"; Package = "rshell-session"; Target = "ssh_smoke"; Tests = @("native_keyboard_interactive_cancel_is_an_authentication_failure") }
            @{ Name = "vault_result_unknown"; Package = "rshell-storage"; Target = "credentials"; Tests = @("put_faults_leave_catalog_old_and_reconcile_cleans_known_or_unknown_result") }
            @{ Name = "database_finalize_failure"; Package = "rshell-storage"; Target = "system_vault"; Features = "test-support"; Tests = @("sqlite_finalize_failure_is_recovered_without_touching_system_vault") }
            @{ Name = "backup_recovery"; Package = "rshell-storage"; Target = "import_legacy"; Tests = @("corrupt_or_missing_primary_recovers_from_sibling_backup_without_mutating_files") }
            @{ Name = "openssh_wildcard_include_cycle"; Package = "rshell-storage"; Target = "import_openssh"; Features = "test-support"; Tests = @("relative_includes_quoted_values_and_globs_are_static_and_deterministic", "include_cycles_and_depth_limits_are_explicit_errors") }
            @{ Name = "repeated_shutdown_reconnect"; Package = "rshell-session"; Target = "actor_lifecycle"; Tests = @("reconnects_are_serial_and_duplicate_shutdown_is_idempotent") }
            @{ Name = "argv_injection"; Package = "rshell-session"; Target = "system_ssh"; Tests = @("argv_is_strict_separate_and_places_destination_after_option_terminator") }
            @{ Name = "secret_unchanged_clear"; Package = "rshell-ui"; Target = "connection_view_models"; Tests = @("existing_profile_secret_policy_never_reinterprets_or_retains_stale_credentials", "public_key_empty_secret_clears_only_an_existing_passphrase") }
            @{ Name = "actor_panic_gtk_survival"; Package = "rshell"; Target = "actor_panic_gtk_survival"; Tests = @("actor_panic_keeps_realized_main_window_alive") }
            @{ Name = "eof_clean_exit"; Package = "rshell-session"; Target = "system_ssh"; Tests = @("fake_ssh_receives_literal_argv_and_clean_eof_preserves_output") }
            @{ Name = "latest_frame_wins"; Package = "rshell-ui"; Target = "terminal_view_model"; Tests = @("stale_and_equal_frames_are_dropped_and_dirty_rows_track_stable_content") }
            @{ Name = "portable_paths"; Package = "rshell-platform"; Tests = @("environment::tests::portable_runtime_paths_are_prepended_once") }
        )
        foreach ($case in $regressionCases) {
            if ($case.Name -eq "portable_paths" -and (-not $platformIsWindows)) {
                continue
            }
            elseif ($platformIsMacOS -and $case.Name -eq "actor_panic_gtk_survival") {
                Invoke-MainThreadGtkRegression
            }
            else {
                Invoke-RegressionCase @case
            }
        }

        [void](Invoke-CapturedChild `
                -Name "release-binary" `
                -FilePath $cargo `
                -Arguments @("build", "--release", "--locked", "--bin", "rshell") `
                -Environment $baseEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-release-build.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-release-build.stderr.log") `
                -TimeoutSeconds 1200)
        $releaseName = "rshell$binarySuffix"
        $releaseBinary = Join-Path (Join-Path (Join-Path $repoRoot "target") "release") $releaseName
        if (!(Test-Path -LiteralPath $releaseBinary -PathType Leaf) -or
            (Get-Item -LiteralPath $releaseBinary).Length -eq 0) {
            throw "release binary was not produced"
        }
        $dependencyLog = Join-Path $artifactRoot "$stem-release-dependencies.stdout.log"
        [void](Invoke-CapturedChild `
                -Name "release-dependencies" `
                -FilePath $cargo `
                -Arguments @("tree", "--locked", "-e", "normal") `
                -Environment $baseEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath $dependencyLog `
                -StderrPath (Join-Path $artifactRoot "$stem-release-dependencies.stderr.log") `
                -TimeoutSeconds 240)
        $dependencyText = [System.IO.File]::ReadAllText($dependencyLog)
        $lockText = [System.IO.File]::ReadAllText((Join-Path $repoRoot "Cargo.lock"))
        if ($dependencyText -match "(?i)libssh2|wezterm-ssh" -or
            $lockText -match "(?i)libssh2|wezterm-ssh|05343b") {
            throw "release dependency scan found a removed dependency"
        }
        Add-Phase "regression_release_no_legacy_dependencies"
    }

    $directObservation = @{
        native_password = Join-Path $observationRoot "native_password.json"
        native_key = Join-Path $observationRoot "native_key.json"
        native_keyboard_interactive = Join-Path $observationRoot "native_keyboard_interactive.json"
        system_agent = Join-Path $observationRoot "system_agent.json"
        host_key = Join-Path $observationRoot "host_key.json"
        vault = Join-Path $observationRoot "vault.json"
    }

    if ($needsSsh) {
        $sshEnvironment = @{}
        foreach ($entry in $baseEnvironment.GetEnumerator()) { $sshEnvironment[$entry.Key] = $entry.Value }
        $sshEnvironment.RSHELL_QA_OBSERVATION_NATIVE_PASSWORD_PATH = $directObservation.native_password
        $sshEnvironment.RSHELL_QA_OBSERVATION_NATIVE_KEY_PATH = $directObservation.native_key
        $sshEnvironment.RSHELL_QA_OBSERVATION_NATIVE_KEYBOARD_INTERACTIVE_PATH = $directObservation.native_keyboard_interactive
        $sshEnvironment.RSHELL_QA_OBSERVATION_HOST_KEY_PATH = $directObservation.host_key
        [void](Invoke-CapturedChild `
                -Name "ssh-native" `
                -FilePath $cargo `
                -Arguments @("test", "--locked", "-p", "rshell-session", "--test", "ssh_smoke", "--", "--nocapture") `
                -Environment $sshEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-ssh-native.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-ssh-native.stderr.log") `
                -TimeoutSeconds 300)
        foreach ($surface in @("native_password", "native_key", "native_keyboard_interactive")) {
            Assert-Observation $directObservation[$surface] $surface @("server_authentication", "server_channel")
        }
        Assert-Observation $directObservation.host_key "host_key" @("server_host_key_prompt")
        Add-Phase "ssh_native"

        $agentBaseline = Get-AgentIdentitySnapshot $sshAdd
        Write-Utf8File $agentCleanupLedger (([ordered]@{
                    kind = "agent-cleanup-ledger"
                    private_key = $agentPrivateKey
                    public_key = $agentPublicKey
                    cleanup_required = $false
                }) | ConvertTo-Json)
        [void](Invoke-CapturedChild `
                -Name "agent-keygen-parent" `
                -FilePath $sshKeygen `
                -Arguments @("-q", "-t", "ed25519", "-N", "", "-f", $agentPrivateKey) `
                -Environment $baseEnvironment `
                -WorkingDirectory $tempRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-agent-keygen.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-agent-keygen.stderr.log") `
                -TimeoutSeconds 30)
        if (-not (Test-Path -LiteralPath $agentPublicKey -PathType Leaf)) {
            throw "The parent-owned agent public key was not generated."
        }
        Write-Utf8File $agentCleanupLedger (([ordered]@{
                    kind = "agent-cleanup-ledger"
                    private_key = $agentPrivateKey
                    public_key = $agentPublicKey
                    cleanup_required = $true
                }) | ConvertTo-Json)
        $agentCleanupRequired = $true
        [void](Invoke-CapturedChild `
                -Name "agent-add-parent" `
                -FilePath $sshAdd `
                -Arguments @($agentPrivateKey) `
                -Environment $baseEnvironment `
                -WorkingDirectory $tempRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-agent-add.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-agent-add.stderr.log") `
                -TimeoutSeconds 15)
        $agentExpectedWithQa = Get-AgentIdentitySnapshot $sshAdd
        $agentEnvironment = @{}
        foreach ($entry in $baseEnvironment.GetEnumerator()) { $agentEnvironment[$entry.Key] = $entry.Value }
        $agentEnvironment.RSHELL_QA_OBSERVATION_SYSTEM_AGENT_PATH = $directObservation.system_agent
        [void](Invoke-CapturedChild `
                -Name "ssh-system-agent" `
                -FilePath $cargo `
                -Arguments @(
                    "test", "--locked", "-p", "rshell-session", "--test", "ssh_smoke",
                    "system_openssh_agent_authenticates_against_local_server", "--", "--ignored", "--exact", "--nocapture"
                ) `
                -Environment $agentEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-ssh-agent.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-ssh-agent.stderr.log") `
                -TimeoutSeconds 180)
        Assert-Observation $directObservation.system_agent "system_agent" @("server_authentication", "server_channel")
        if (Compare-Object $agentExpectedWithQa (Get-AgentIdentitySnapshot $sshAdd)) {
            throw "The direct system OpenSSH smoke changed the parent-owned agent identity set."
        }
        Add-Phase "ssh_system_agent"
    }

    if ($needsVault) {
        Write-Utf8File $vaultCleanupLedger (([ordered]@{
                    kind = "vault-cleanup-ledger"
                    references = @($vaultReference, $vaultFailureReference)
                }) | ConvertTo-Json)
        $vaultCleanupRequired = $true
        if ($Mode -eq "All") {
            $failureVaultEnvironment = @{}
            foreach ($entry in $baseEnvironment.GetEnumerator()) { $failureVaultEnvironment[$entry.Key] = $entry.Value }
            $failureVaultEnvironment.RSHELL_P0_QA_VAULT_REFERENCE = $vaultFailureReference
            $failureVaultEnvironment.RSHELL_P0_QA_VAULT_FAILURE_SECRET = $vaultFailureSecretValue
            $failureExit = Invoke-CapturedChild `
                -Name "fail_during_vault_probe" `
                -FilePath $cargo `
                -Arguments @(
                    "test", "--locked", "-p", "rshell-storage", "--features", "test-support",
                    "--test", "system_vault", "system_vault_failure_probe_leaves_exact_parent_entry_for_harness_cleanup",
                    "--", "--ignored", "--exact", "--nocapture"
                ) `
                -Environment $failureVaultEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-vault-failure.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-vault-failure.stderr.log") `
                -TimeoutSeconds 120 `
                -AllowFailure $true
            if ($failureExit -eq 0) {
                throw "The deterministic fail_during_vault_probe did not fail."
            }
            [void](Invoke-CapturedChild `
                    -Name "vault-failure-ledger-cleanup" `
                    -FilePath $cargo `
                    -Arguments @(
                        "test", "--locked", "-p", "rshell-storage", "--features", "test-support",
                        "--test", "system_vault", "system_vault_cleanup_exact_parent_reference",
                        "--", "--ignored", "--exact", "--nocapture"
                    ) `
                    -Environment $failureVaultEnvironment `
                    -WorkingDirectory $repoRoot `
                    -StdoutPath (Join-Path $artifactRoot "$stem-vault-failure-cleanup.stdout.log") `
                    -StderrPath (Join-Path $artifactRoot "$stem-vault-failure-cleanup.stderr.log") `
                    -TimeoutSeconds 120)
            Add-Phase "fail_during_vault_probe"
        }
        $vaultEnvironment = @{}
        foreach ($entry in $baseEnvironment.GetEnumerator()) { $vaultEnvironment[$entry.Key] = $entry.Value }
        $vaultEnvironment.RSHELL_P0_QA_VAULT_OBSERVATION_PATH = $directObservation.vault
        $vaultEnvironment.RSHELL_P0_QA_VAULT_REFERENCE = $vaultReference
        [void](Invoke-CapturedChild `
                -Name "vault-real-os" `
                -FilePath $cargo `
                -Arguments @(
                    "test", "--locked", "-p", "rshell-storage", "--features", "test-support",
                    "--test", "system_vault", "system_vault_real_os_probe_uses_coordinator_and_cleans_random_entry",
                    "--", "--ignored", "--exact", "--nocapture"
                ) `
                -Environment $vaultEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-vault.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-vault.stderr.log") `
                -TimeoutSeconds 180)
        Assert-Observation $directObservation.vault "vault" @(
            "vault_credential_reference", "vault_database_secret_scan",
            "vault_temporary_reference_zero", "journal_count_zero"
        )
        Add-Phase "vault_real_os"
    }

    if ($needsGtk) {
        $encryptedKey = Join-Path $tempRoot "encrypted-client-key"
        [void](Invoke-CapturedChild `
                -Name "build-askpass" `
                -FilePath $cargo `
                -Arguments @(
                    "build", "--locked", "-p", "rshell-session",
                    "--bin", "rshell-qa-askpass", "--bin", "rshell-p0-tui"
                ) `
                -Environment $baseEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-askpass-build.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-askpass-build.stderr.log") `
                -TimeoutSeconds 180)
        $debugRoot = Join-Path (Join-Path $repoRoot "target") "debug"
        $askpass = Join-Path $debugRoot "rshell-qa-askpass$binarySuffix"
        $tuiFixture = Join-Path $debugRoot "rshell-p0-tui$binarySuffix"
        if (-not (Test-Path -LiteralPath $askpass -PathType Leaf)) {
            throw "The QA askpass helper was not built."
        }
        if (-not (Test-Path -LiteralPath $tuiFixture -PathType Leaf)) {
            throw "The local TUI fixture was not built."
        }
        $keygenEnvironment = @{}
        foreach ($entry in $childEnvironment.GetEnumerator()) { $keygenEnvironment[$entry.Key] = $entry.Value }
        $keygenEnvironment.SSH_ASKPASS = $askpass
        $keygenEnvironment.SSH_ASKPASS_REQUIRE = "force"
        if ([string]::IsNullOrWhiteSpace($env:DISPLAY)) {
            $keygenEnvironment.DISPLAY = "rshell-p0-askpass"
        }
        $keygenEnvironment.RSHELL_QA_ASKPASS_SECRET_ENV = $passphraseName
        [void](Invoke-CapturedChild `
                -Name "keygen-encrypted" `
                -FilePath $sshKeygen `
                -Arguments @("-q", "-t", "ed25519", "-f", $encryptedKey) `
                -Environment $keygenEnvironment `
                -WorkingDirectory $tempRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-keygen.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-keygen.stderr.log") `
                -TimeoutSeconds 30)
        if (-not (Test-Path -LiteralPath $encryptedKey -PathType Leaf) -or
            -not (Test-Path -LiteralPath "$encryptedKey.pub" -PathType Leaf)) {
            throw "The disposable encrypted SSH key pair was not generated."
        }
        Add-Phase "encrypted_key_generation"

        $fixtureReady = Join-Path $tempRoot "fixture.ready.json"
        $fixtureEnvironment = @{}
        foreach ($entry in $childEnvironment.GetEnumerator()) { $fixtureEnvironment[$entry.Key] = $entry.Value }
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_READY_PATH = $fixtureReady
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_STOP_PATH = $fixtureStop
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_OBSERVATION_DIR = $fixtureObservationRoot
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_ENCRYPTED_KEY_PATH = $encryptedKey
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_PASSWORD_ENV = $passwordName
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_KEY_PASSPHRASE_ENV = $passphraseName
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_KBI_VISIBLE_ANSWER_ENV = $kbiVisibleName
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_KBI_ONE_TIME_CODE_ENV = $kbiCodeName
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_EXPECTED_SURFACES = "native_password,native_key,native_keyboard_interactive,system_agent,host_key"
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_AGENT_PUBLIC_KEY_PATH = $agentPublicKey
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_RUN_NONCE = $runId
        $fixtureEnvironment.RSHELL_QA_SSH_SMOKE_FIXTURE_ID = "fixture-$runId"

        $sshSmokePattern = if ($platformIsWindows) { '^ssh_smoke-[0-9a-f]+\.exe$' } else { '^ssh_smoke-[0-9a-f]+$' }
        $sshSmokeBinary = Get-ChildItem -LiteralPath (Join-Path $debugRoot "deps") `
            -File |
            Where-Object { $_.Name -match $sshSmokePattern } |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if ($null -eq $sshSmokeBinary) {
            throw "The compiled SSH smoke fixture executable is unavailable."
        }
        if ($Mode -eq "All") {
            $failureFixtureEnvironment = @{}
            foreach ($entry in $fixtureEnvironment.GetEnumerator()) { $failureFixtureEnvironment[$entry.Key] = $entry.Value }
            $failureFixtureEnvironment.RSHELL_QA_INJECT_FAIL_BEFORE_READY = "1"
            $failureFixtureEnvironment.RSHELL_QA_SSH_SMOKE_READY_PATH = Join-Path $tempRoot "failure-fixture.ready.json"
            $fixtureFailureRun = Start-CapturedChild `
                -Name "fail_before_fixture_ready" `
                -FilePath $sshSmokeBinary.FullName `
                -Arguments @("local_russh_smoke_fixture_server", "--ignored", "--exact", "--nocapture") `
                -Environment $failureFixtureEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-fixture-failure.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-fixture-failure.stderr.log")
            $fixtureFailureExit = Complete-CapturedChild -Run $fixtureFailureRun -TimeoutSeconds 30 -AllowFailure $true
            if ($fixtureFailureExit -eq 0 -or
                (Test-Path -LiteralPath $failureFixtureEnvironment.RSHELL_QA_SSH_SMOKE_READY_PATH)) {
                throw "The deterministic fail_before_fixture_ready probe did not fail before readiness."
            }
            Add-Phase "fail_before_fixture_ready"

            $nonzeroObservationRoot = Join-Path $tempRoot "fixture-nonzero-observations"
            [void](New-Item -ItemType Directory -Path $nonzeroObservationRoot)
            $nonzeroReady = Join-Path $tempRoot "fixture-nonzero.ready.json"
            $nonzeroStop = Join-Path $tempRoot "fixture-nonzero.stop"
            $nonzeroEnvironment = @{}
            foreach ($entry in $fixtureEnvironment.GetEnumerator()) { $nonzeroEnvironment[$entry.Key] = $entry.Value }
            $nonzeroEnvironment.RSHELL_QA_INJECT_FINAL_FAILURE = "1"
            $nonzeroEnvironment.RSHELL_QA_SSH_SMOKE_READY_PATH = $nonzeroReady
            $nonzeroEnvironment.RSHELL_QA_SSH_SMOKE_STOP_PATH = $nonzeroStop
            $nonzeroEnvironment.RSHELL_QA_SSH_SMOKE_OBSERVATION_DIR = $nonzeroObservationRoot
            $fixtureNonzeroRun = Start-CapturedChild `
                -Name "fixture_nonzero_shutdown" `
                -FilePath $sshSmokeBinary.FullName `
                -Arguments @("local_russh_smoke_fixture_server", "--ignored", "--exact", "--nocapture") `
                -Environment $nonzeroEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-fixture-nonzero.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-fixture-nonzero.stderr.log")
            [void](Wait-ForFixtureReady -Run $fixtureNonzeroRun -ReadyPath $nonzeroReady)
            Write-Utf8File $nonzeroStop "stop`n"
            $nonzeroExit = Complete-CapturedChild -Run $fixtureNonzeroRun -TimeoutSeconds 30 -AllowFailure $true
            if ($nonzeroExit -eq 0) {
                throw "The deterministic fixture_nonzero_shutdown probe did not report its final assertion failure."
            }
            Add-Phase "fixture_nonzero_shutdown"
        }
        $fixtureRun = Start-CapturedChild `
            -Name "ssh-fixture" `
            -FilePath $sshSmokeBinary.FullName `
            -Arguments @("local_russh_smoke_fixture_server", "--ignored", "--exact", "--nocapture") `
            -Environment $fixtureEnvironment `
            -WorkingDirectory $repoRoot `
            -StdoutPath (Join-Path $artifactRoot "$stem-fixture.stdout.log") `
            -StderrPath (Join-Path $artifactRoot "$stem-fixture.stderr.log")
        $ready = Wait-ForFixtureReady -Run $fixtureRun -ReadyPath $fixtureReady
        if ($ready.version -ne 1 -or $ready.generated_by -ne "p0_qa") {
            throw "The local russh fixture readiness contract is invalid."
        }

        $openSshImport = Join-Path $tempRoot "openssh-cancel.conf"
        Write-Utf8File $openSshImport "Host p0-cancel`n  HostName cancel.example.test`n  User operator`n"
        $legacyImport = Join-Path (Join-Path (Join-Path (Join-Path $repoRoot "tests") "fixtures") "smoke") "legacy.json"
        $quotedTuiFixture = $tuiFixture.Replace("'", "''")
        $actions = [System.Collections.Generic.List[object]]::new()
        Set-ActionBinding -Surface "gtk"
        Add-Action $actions ([ordered]@{ action = "wait_window_realized" })
        Set-ActionBinding -Surface "local_terminal" -Connection "local"
        Add-Action $actions ([ordered]@{ action = "new_tab" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "P0-LOCAL-READY" })
        Add-Action $actions ([ordered]@{
                action = "send_terminal_text"
                text = "[Console]::Write(([char]27).ToString() + '[31mp0-color' + ([char]27).ToString() + '[0m' + [Environment]::NewLine); Write-Output 'p0-wide-界'`r"
                expected_color_marker = "p0-color"
            })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "`$p0Marker = -join [char[]](112,48,45,112,97,115,116,101,45,101,102,102,101,99,116); `$null = Read-Host -AsSecureString -Prompt 'p0-paste-prompt'; Write-Output `$p0Marker`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "p0-paste-prompt" })
        Add-Action $actions ([ordered]@{ action = "paste_text_from_env"; env_var = $pasteName; effect_marker = "p0-paste-effect" })
        Add-Action $actions ([ordered]@{ action = "resize_terminal"; width = 960; height = 640; scale = 1.0 })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "p0-wide-界" })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "& '$quotedTuiFixture'`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "P0-TUI-ENTERED" })
        Add-Action $actions ([ordered]@{ action = "search_terminal"; text = "P0-TUI"; case_sensitive = $true; regex = $false })
        Add-Action $actions ([ordered]@{
                action = "select_range"; start_x = 22.5; start_y = 9.0; end_x = 27.1; end_y = 9.0
                rectangular = $false; expected_text = "界"; expect_wide_midpoint = $true
            })
        Add-Action $actions ([ordered]@{ action = "copy_selection" })
        # The fixture intentionally leaves the console in cooked mode, so Enter commits the quit key.
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "q`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "P0-TUI-EXITED" })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "Write-Output P0-TUI-POST-EXIT-FRAME`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "P0-TUI-POST-EXIT-FRAME" })

        Set-ActionBinding -Surface "tabs_splits" -Connection "local"
        Add-Action $actions ([ordered]@{ action = "new_tab" })
        Add-Action $actions ([ordered]@{ action = "new_tab" })
        Add-Action $actions ([ordered]@{ action = "split_horizontal" })
        Add-Action $actions ([ordered]@{ action = "split_vertical" })
        Add-Action $actions ([ordered]@{ action = "switch_tab"; tab = 0 })

        Add-ConnectionPrefix $actions "native_password" $ready.endpoints.native_password "native_ssh" "password" "native_password"
        Add-Action $actions ([ordered]@{ action = "set_connection_field"; field = [ordered]@{ kind = "secret_from_env"; env_var = $passwordName } })
        Add-SubmitConnect $actions "native_password"
        Add-Action $actions ([ordered]@{ action = "respond_host_key"; accept = $true })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "gui-native-password-$runId`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "gui-native-password-$runId" })
        Set-ActionBinding -Surface "tabs_splits" -Connection "native_password"
        Add-Action $actions ([ordered]@{ action = "reconnect" })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "gui-native-reconnect-$runId`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "gui-native-reconnect-$runId" })

        Set-ActionBinding -Surface "gtk"
        Add-Action $actions ([ordered]@{ action = "visual_checkpoint" })

        Add-ConnectionPrefix $actions "native_key" $ready.endpoints.native_key "native_ssh" "public_key" "native_key"
        Add-TextFieldAction $actions "identity_file" $encryptedKey
        Add-Action $actions ([ordered]@{ action = "set_connection_field"; field = [ordered]@{ kind = "secret_from_env"; env_var = $passphraseName } })
        Add-SubmitConnect $actions "native_key"
        Add-Action $actions ([ordered]@{ action = "respond_host_key"; accept = $true })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "gui-native-key-$runId`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "gui-native-key-$runId" })

        Add-ConnectionPrefix $actions "native_keyboard_interactive" $ready.endpoints.native_keyboard_interactive "native_ssh" "keyboard_interactive" "native_keyboard_interactive"
        Add-SubmitConnect $actions "native_keyboard_interactive"
        Add-Action $actions ([ordered]@{ action = "respond_host_key"; accept = $true })
        Add-Action $actions ([ordered]@{ action = "respond_auth"; prompt = 0; env_var = $kbiVisibleName })
        Add-Action $actions ([ordered]@{ action = "respond_auth"; prompt = 1; env_var = $kbiCodeName })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "gui-native-kbi-$runId`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "gui-native-kbi-$runId" })

        Add-ConnectionPrefix $actions "system_agent" $ready.endpoints.system_agent "system_open_ssh" "agent" "system_agent"
        Add-SubmitConnect $actions "system_agent"
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "yes`r" })
        Add-Action $actions ([ordered]@{ action = "send_terminal_text"; text = "gui-system-agent-$runId`r" })
        Add-Action $actions ([ordered]@{ action = "wait_frame_contains"; text = "gui-system-agent-$runId" })

        Add-ConnectionPrefix $actions "host_key" $ready.endpoints.host_key "native_ssh" "password" "host_key"
        Add-Action $actions ([ordered]@{ action = "set_connection_field"; field = [ordered]@{ kind = "secret_from_env"; env_var = $passwordName } })
        Add-SubmitConnect $actions "host_key"
        Add-Action $actions ([ordered]@{ action = "respond_host_key"; accept = $false })

        Add-ConnectionPrefix $actions "vault" $ready.endpoints.native_password "native_ssh" "password" "vault"
        Add-Action $actions ([ordered]@{ action = "set_connection_field"; field = [ordered]@{ kind = "secret_from_env"; env_var = $passwordName } })
        Add-Action $actions ([ordered]@{ action = "submit_connection" })

        Set-ActionBinding -Surface "imports" -Connection "legacy_import"
        Add-Action $actions ([ordered]@{
                action = "preview_import"; source = "legacy_rshell_json"; path = $legacyImport
                expected = [ordered]@{
                    groups = 1; connections = 1; group_name = "P0 imported"; connection_name = "P0 legacy import"
                    host = "legacy.example.test"; authentication = "agent"; credential_reference_present = $false
                    terminal_override_present = $false; importable = $true; wildcard = $false
                }
            })
        Add-Action $actions ([ordered]@{ action = "commit_import" })
        Set-ActionBinding -Surface "imports" -Connection "openssh_cancel"
        Add-Action $actions ([ordered]@{
                action = "preview_import"; source = "open_ssh_config"; path = $openSshImport
                expected = [ordered]@{
                    groups = 0; connections = 1; group_name = ""; connection_name = "p0-cancel"
                    host = "cancel.example.test"; authentication = "agent"; credential_reference_present = $false
                    terminal_override_present = $false; importable = $true; wildcard = $false
                }
            })
        Add-Action $actions ([ordered]@{ action = "cancel_import" })
        Set-ActionBinding -Surface "cleanup"
        Add-Action $actions ([ordered]@{ action = "close_all" })

        $scenarioPath = Join-Path $tempRoot "p0-scenario.json"
        $scenario = [ordered]@{
            version = 1
            run_nonce = $runId
            step_timeout_ms = 10000
            scenario_timeout_ms = 120000
            external_observations = @(
                [ordered]@{ surface = "native_password"; path = (Join-Path $fixtureObservationRoot "native_password.json"); fixture = "fixture-$runId"; connection = "native_password"; endpoint = "$($ready.endpoints.native_password.host):$($ready.endpoints.native_password.port)" }
                [ordered]@{ surface = "native_key"; path = (Join-Path $fixtureObservationRoot "native_key.json"); fixture = "fixture-$runId"; connection = "native_key"; endpoint = "$($ready.endpoints.native_key.host):$($ready.endpoints.native_key.port)" }
                [ordered]@{ surface = "native_keyboard_interactive"; path = (Join-Path $fixtureObservationRoot "native_keyboard_interactive.json"); fixture = "fixture-$runId"; connection = "native_keyboard_interactive"; endpoint = "$($ready.endpoints.native_keyboard_interactive.host):$($ready.endpoints.native_keyboard_interactive.port)" }
                [ordered]@{ surface = "system_agent"; path = (Join-Path $fixtureObservationRoot "system_agent.json"); fixture = "fixture-$runId"; connection = "system_agent"; endpoint = "$($ready.endpoints.system_agent.host):$($ready.endpoints.system_agent.port)" }
                [ordered]@{ surface = "host_key"; path = (Join-Path $fixtureObservationRoot "host_key.json"); fixture = "fixture-$runId"; connection = "host_key"; endpoint = "$($ready.endpoints.host_key.host):$($ready.endpoints.host_key.port)" }
            )
            actions = $actions
        }
        Write-Utf8File $scenarioPath ($scenario | ConvertTo-Json -Depth 12)

        $guiEnvironment = @{}
        foreach ($entry in $childEnvironment.GetEnumerator()) { $guiEnvironment[$entry.Key] = $entry.Value }
        $guiHome = Join-Path $tempRoot "gui-home"
        [void](New-Item -ItemType Directory -Path (Join-Path $guiHome ".ssh") -Force)
        $guiEnvironment.HOME = $guiHome
        $guiEnvironment.USERPROFILE = $guiHome
        if (-not $platformIsWindows) {
            $guiXdgConfig = Join-Path $guiHome ".config"
            [void](New-Item -ItemType Directory -Path $guiXdgConfig -Force)
            $guiEnvironment.XDG_CONFIG_HOME = $guiXdgConfig
        }
        $guiEnvironment.CARGO_HOME = $cargoHome
        if ($rustupHome) { $guiEnvironment.RUSTUP_HOME = $rustupHome }
        $shellProfileOutput = Join-Path $artifactRoot "$stem-shell-profile-path.stdout.log"
        [void](Invoke-CapturedChild `
                -Name "shell-profile-path" `
                -FilePath $pwsh `
                -Arguments @("-NoLogo", "-NoProfile", "-NonInteractive", "-Command", '[Console]::Out.Write($PROFILE.CurrentUserCurrentHost)') `
                -Environment $guiEnvironment `
                -WorkingDirectory $tempRoot `
                -StdoutPath $shellProfileOutput `
                -StderrPath (Join-Path $artifactRoot "$stem-shell-profile-path.stderr.log") `
                -TimeoutSeconds 30)
        $shellProfilePath = [System.IO.Path]::GetFullPath((Get-Content -LiteralPath $shellProfileOutput -Raw).Trim())
        $guiHomePrefix = [System.IO.Path]::GetFullPath($guiHome).TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
        $profileComparison = if ($platformIsWindows) { [System.StringComparison]::OrdinalIgnoreCase } else { [System.StringComparison]::Ordinal }
        if (-not $shellProfilePath.StartsWith($guiHomePrefix, $profileComparison)) {
            throw "The isolated PowerShell profile path escaped the temporary GUI home."
        }
        [void](New-Item -ItemType Directory -Path (Split-Path -Parent $shellProfilePath) -Force)
        $shellProfile = @'
Register-EngineEvent -SourceIdentifier PowerShell.OnIdle -MaxTriggerCount 1 -Action {
    [Console]::WriteLine('P0-LOCAL-READY')
} | Out-Null
function global:prompt { 'P0> ' }
'@
        Write-Utf8File $shellProfilePath $shellProfile
        $guiReportTemp = Join-Path $tempRoot "production-p0-report.json"
        $guiPngTemp = [System.IO.Path]::ChangeExtension($guiReportTemp, ".png")
        $guiExit = Invoke-CapturedChild `
                -Name "gtk-production" `
                -FilePath $cargo `
                -Arguments @("run", "--locked", "--", "--smoke-p0", $scenarioPath, $guiReportTemp) `
                -Environment $guiEnvironment `
                -WorkingDirectory $repoRoot `
                -StdoutPath (Join-Path $artifactRoot "$stem-gtk.stdout.log") `
                -StderrPath (Join-Path $artifactRoot "$stem-gtk.stderr.log") `
                -TimeoutSeconds 240 `
                -AllowFailure $true

        if (Test-Path -LiteralPath $guiReportTemp -PathType Leaf) {
            $pendingReport = Get-Content -LiteralPath $guiReportTemp -Raw | ConvertFrom-Json
        }
        if (Test-Path -LiteralPath $guiPngTemp -PathType Leaf) {
            $pendingDimensions = Assert-Png $guiPngTemp
            $pendingPngBytes = [System.IO.File]::ReadAllBytes($guiPngTemp)
        }
        if ($guiExit -ne 0) {
            throw "P0 smoke phase 'gtk-production' failed with exit $guiExit; inspect the retained failed report and redacted artifact logs."
        }
        Assert-VisualContract -Report $pendingReport -PngInfo $pendingDimensions
        Add-Phase "gtk_production"

        Write-Utf8File $fixtureStop "stop`n"
        $fixtureStopExit = Complete-CapturedChild -Run $fixtureRun -TimeoutSeconds 30 -AllowFailure $true
        $fixtureRun = $null
        if ($fixtureStopExit -ne 0) {
            throw "The local russh fixture final assertions failed with exit $fixtureStopExit."
        }
        foreach ($surface in @("native_password", "native_key", "native_keyboard_interactive", "system_agent")) {
            $fixtureEndpoint = $ready.endpoints.$surface
            Assert-Observation `
                (Join-Path $fixtureObservationRoot "$surface.json") `
                $surface `
                @("server_authentication", "server_channel") `
                @{ run_nonce = $runId; fixture = "fixture-$runId"; connection = $surface; endpoint = "$($fixtureEndpoint.host):$($fixtureEndpoint.port)" }
        }
        Assert-Observation `
            (Join-Path $fixtureObservationRoot "host_key.json") `
            "host_key" `
            @("server_host_key_prompt") `
            @{ run_nonce = $runId; fixture = "fixture-$runId"; connection = "host_key"; endpoint = "$($ready.endpoints.host_key.host):$($ready.endpoints.host_key.port)" }
        Add-Phase "ssh_fixture_cleanup"

        if (Compare-Object $agentExpectedWithQa (Get-AgentIdentitySnapshot $sshAdd)) {
            throw "The local russh fixture changed the parent-owned system OpenSSH agent set."
        }

        if (-not (Test-Path -LiteralPath $guiReportTemp -PathType Leaf)) {
            throw "The production P0 JSON report is missing."
        }
        $report = $pendingReport
        if ($report.state -ne "passed") {
            throw "The production P0 report did not pass."
        }
        foreach ($surface in $surfaceNames) {
            if ($report.$surface.status -ne "passed") {
                throw "P0 smoke contract failure: missing evidence field '$surface' (mode=$mode, platform=$platform)"
            }
        }
        if (@($report.steps | Where-Object { $_.state -eq "skipped" }).Count -ne 0) {
            throw "The production P0 report contains skipped actions."
        }
        if (@($report.steps | Where-Object { $_.state -ne "passed" }).Count -ne 0) {
            throw "The production P0 report contains a non-passed action."
        }
        $pendingReport = $report
    }
    else {
        $pendingReport = [pscustomobject]([ordered]@{
            version = 1
            platform = $platform
            mode = $mode
            state = "passed"
            phases = $phases
        })
    }
}
catch {
    if ($RegressionCaseProbe) {
        [Console]::Error.WriteLine($_.Exception.Message)
        throw
    }
    $failure = $_.Exception.Message
}
finally {
    if ($null -ne $fixtureRun) {
        try {
            if (-not (Test-Path -LiteralPath $fixtureStop)) {
                Write-Utf8File $fixtureStop "stop`n"
            }
            $fixtureFinallyExit = Complete-CapturedChild -Run $fixtureRun -TimeoutSeconds 30 -AllowFailure $true
            $fixtureRun = $null
            if ($fixtureFinallyExit -ne 0) {
                if ($null -eq $failure) { $failure = "The local russh fixture final assertions failed during cleanup." }
            }
        }
        catch {
            try { $fixtureRun.Process.Kill($true) } catch {}
            if ($null -eq $failure) { $failure = "The local russh fixture could not be cleaned." }
        }
    }

    if ($null -ne $agentBaseline) {
        try {
            if ($agentCleanupRequired -and (Test-Path -LiteralPath $agentPublicKey -PathType Leaf)) {
                [void](Invoke-CapturedChild `
                        -Name "agent-ledger-cleanup" `
                        -FilePath $sshAdd `
                        -Arguments @("-d", $agentPublicKey) `
                        -Environment $baseEnvironment `
                        -WorkingDirectory $repoRoot `
                        -StdoutPath (Join-Path $artifactRoot "$stem-agent-cleanup.stdout.log") `
                        -StderrPath (Join-Path $artifactRoot "$stem-agent-cleanup.stderr.log") `
                        -TimeoutSeconds 15 `
                        -AllowFailure $true)
            }
            if (Compare-Object $agentBaseline (Get-AgentIdentitySnapshot $sshAdd)) {
                if ($null -eq $failure) { $failure = "The parent-ledger system OpenSSH identity was not removed exactly." }
            }
            else {
                if ($Mode -eq "All" -and $null -eq $failure) {
                    $lostReplyScript = Join-Path $tempRoot "agent-add-lost-reply.ps1"
                    Write-Utf8File $lostReplyScript @"
& '$($sshAdd.Replace("'", "''"))' '$($agentPrivateKey.Replace("'", "''"))'
if (`$LASTEXITCODE -ne 0) { exit `$LASTEXITCODE }
exit 91
"@
                    $lostReplyExit = Invoke-CapturedChild `
                        -Name "agent_add_lost_reply" `
                        -FilePath $pwsh `
                        -Arguments @("-NoProfile", "-File", $lostReplyScript) `
                        -Environment $baseEnvironment `
                        -WorkingDirectory $tempRoot `
                        -StdoutPath (Join-Path $artifactRoot "$stem-agent-lost-reply.stdout.log") `
                        -StderrPath (Join-Path $artifactRoot "$stem-agent-lost-reply.stderr.log") `
                        -TimeoutSeconds 30 `
                        -AllowFailure $true
                    if ($lostReplyExit -ne 91 -or -not (Compare-Object $agentBaseline (Get-AgentIdentitySnapshot $sshAdd))) {
                        throw "The deterministic agent_add_lost_reply probe did not mutate before losing its reply."
                    }
                    [void](Invoke-CapturedChild `
                            -Name "agent_add_lost_reply_cleanup" `
                            -FilePath $sshAdd `
                            -Arguments @("-d", $agentPrivateKey) `
                            -Environment $baseEnvironment `
                            -WorkingDirectory $tempRoot `
                            -StdoutPath (Join-Path $artifactRoot "$stem-agent-lost-reply-cleanup.stdout.log") `
                            -StderrPath (Join-Path $artifactRoot "$stem-agent-lost-reply-cleanup.stderr.log") `
                            -TimeoutSeconds 15)
                    if (Compare-Object $agentBaseline (Get-AgentIdentitySnapshot $sshAdd)) {
                        throw "The lost-reply agent key was not removed before completing cleanup."
                    }
                    Add-Phase "agent_add_lost_reply"
                }
                Add-Phase "system_agent_cleanup"
            }
        }
        catch {
            if ($null -eq $failure) { $failure = "System OpenSSH agent cleanup verification failed." }
        }
    }

    if ($vaultCleanupRequired) {
        foreach ($reference in @($vaultReference, $vaultFailureReference)) {
            try {
                $vaultCleanupEnvironment = @{}
                foreach ($entry in $baseEnvironment.GetEnumerator()) { $vaultCleanupEnvironment[$entry.Key] = $entry.Value }
                $vaultCleanupEnvironment.RSHELL_P0_QA_VAULT_REFERENCE = $reference
                [void](Invoke-CapturedChild `
                        -Name "vault-ledger-cleanup" `
                        -FilePath $cargo `
                        -Arguments @(
                            "test", "--locked", "-p", "rshell-storage", "--features", "test-support",
                            "--test", "system_vault", "system_vault_cleanup_exact_parent_reference",
                            "--", "--ignored", "--exact", "--nocapture"
                        ) `
                        -Environment $vaultCleanupEnvironment `
                        -WorkingDirectory $repoRoot `
                        -StdoutPath (Join-Path $artifactRoot "$stem-vault-ledger-cleanup.stdout.log") `
                        -StderrPath (Join-Path $artifactRoot "$stem-vault-ledger-cleanup.stderr.log") `
                        -TimeoutSeconds 120)
            }
            catch {
                if ($null -eq $failure) { $failure = "Parent-ledger system vault cleanup failed." }
            }
        }
        if ($null -eq $failure) { Add-Phase "vault_ledger_cleanup" }
    }

    try {
        foreach ($ownedId in $script:ownedChildIds) {
            if ($null -ne (Get-Process -Id $ownedId -ErrorAction SilentlyContinue)) {
                throw "An owned child process remains after cleanup."
            }
        }
        $ownedProcessesClean = $true
        Add-Phase "owned_process_cleanup"
    }
    catch {
        if ($null -eq $failure) { $failure = "Owned process cleanup verification failed." }
    }

    try {
        $scanEnvironment = @{}
        foreach ($entry in $baseEnvironment.GetEnumerator()) { $scanEnvironment[$entry.Key] = $entry.Value }
        foreach ($entry in $secretEnvironment.GetEnumerator()) { $scanEnvironment[$entry.Key] = $entry.Value }
        $scanEnvironment.RSHELL_QA_SECRET_ENV_VARS = $secretNames -join ","
        foreach ($scanRoot in @($artifactRoot, $tempRoot)) {
            [void](Invoke-CapturedChild `
                    -Name "assert-no-secrets" `
                    -FilePath $pwsh `
                    -Arguments @("-NoProfile", "-File", (Join-Path $PSScriptRoot "assert-no-secrets.ps1"), "-ArtifactRoot", $scanRoot) `
                    -Environment $scanEnvironment `
                    -WorkingDirectory $repoRoot `
                    -StdoutPath (Join-Path $artifactRoot "$stem-secret-scan.stdout.log") `
                    -StderrPath (Join-Path $artifactRoot "$stem-secret-scan.stderr.log") `
                    -TimeoutSeconds 120)
        }
        $secretScanRan = $true
    }
    catch {
        if ($null -eq $failure) { $failure = "The artifact secret scan failed; inspect its redacted log." }
    }

    if (Test-Path -LiteralPath $tempRoot -PathType Container) {
        $expectedPrefix = [System.IO.Path]::GetFullPath((Join-Path $temporaryBase "rshell-p0-qa-"))
        $resolvedTemp = [System.IO.Path]::GetFullPath($tempRoot)
        if (-not $resolvedTemp.StartsWith($expectedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            if ($null -eq $failure) { $failure = "P0 temp cleanup target failed its safety check." }
        }
        else {
            Remove-Item -LiteralPath $tempRoot -Recurse -Force
            if (Test-Path -LiteralPath $tempRoot) {
                if ($null -eq $failure) { $failure = "P0 temp cleanup verification failed." }
            }
        }
    }
}

if (-not $secretScanRan -and $null -eq $failure) {
    $failure = "The artifact secret scan did not run."
}
if (-not $ownedProcessesClean -and $null -eq $failure) {
    $failure = "Owned process cleanup did not complete."
}
if ($env:RSHELL_QA_INJECT_LATE_FINALIZATION_FAILURE -eq "1" -and
    $null -ne $pendingReport -and $pendingReport.state -eq "passed") {
    $failure = "Injected late cleanup/security finalization failure."
}
if ($null -ne $failure -and $null -ne $pendingReport) {
    Set-LateFailure -Report $pendingReport -Code "late_cleanup_or_security_failure"
}

$finalizeRoot = Join-Path $temporaryBase "rshell-p0-finalize-$([Guid]::NewGuid().ToString('N'))"
[void](New-Item -ItemType Directory -Path $finalizeRoot)
$finalizationException = $null
try {
    if ($null -ne $failure -and $null -eq $pendingReport) {
        $failed = [ordered]@{
            version = 1; platform = $platform; mode = $mode; state = "failed"
            error = "p0_smoke_failed"; phases = $phases
        }
        foreach ($surface in $surfaceNames) {
            $failed[$surface] = [ordered]@{ status = "failed"; missing_evidence = @("run_failed") }
        }
        $pendingReport = [pscustomobject]$failed
        $pendingPngBytes = $null
        $pendingDimensions = $null
    }
    elseif ($null -ne $failure) {
        if ($null -eq $pendingReport.PSObject.Properties["phases"]) {
            $pendingReport | Add-Member -NotePropertyName phases -NotePropertyValue $phases
        }
    }
    elseif ($null -eq $pendingReport) {
        $failure = "P0 final report was not produced."
        throw $failure
    }

    if ($null -ne $pendingReport.PSObject.Properties["png_path"]) {
        $pendingReport.png_path = $artifactPng
        $pendingReport.requested_png_path = $artifactPng
    }
    $finalJson = $pendingReport | ConvertTo-Json -Depth 20
    $stagedReport = Join-Path $finalizeRoot "report.json"
    Write-Utf8File $stagedReport $finalJson
    if ($null -ne $pendingPngBytes) {
        [System.IO.File]::WriteAllBytes((Join-Path $finalizeRoot "report.png"), $pendingPngBytes)
    }

    $finalScanEnvironment = @{}
    foreach ($entry in $baseEnvironment.GetEnumerator()) { $finalScanEnvironment[$entry.Key] = $entry.Value }
    foreach ($entry in $secretEnvironment.GetEnumerator()) { $finalScanEnvironment[$entry.Key] = $entry.Value }
    $finalScanEnvironment.RSHELL_QA_SECRET_ENV_VARS = $secretNames -join ","
    [void](Invoke-CapturedChild `
            -Name "assert-no-secrets-final" `
            -FilePath $pwsh `
            -Arguments @("-NoProfile", "-File", (Join-Path $PSScriptRoot "assert-no-secrets.ps1"), "-ArtifactRoot", $finalizeRoot) `
            -Environment $finalScanEnvironment `
            -WorkingDirectory $repoRoot `
            -StdoutPath (Join-Path $artifactRoot "$stem-final-secret-scan.stdout.log") `
            -StderrPath (Join-Path $artifactRoot "$stem-final-secret-scan.stderr.log") `
            -TimeoutSeconds 120)

    Write-Utf8File $artifactReport $finalJson
    if ($null -ne $pendingPngBytes) {
        [System.IO.File]::WriteAllBytes($artifactPng, $pendingPngBytes)
    }
    Write-Junit $artifactJunit $pendingReport $pendingDimensions $failure
}
catch {
    $finalizationException = $_.Exception.Message
}
finally {
    if (Test-Path -LiteralPath $finalizeRoot -PathType Container) {
        Remove-Item -LiteralPath $finalizeRoot -Recurse -Force
    }
}

if ($null -ne $finalizationException) {
    $failure = "P0 artifact finalization failed after cleanup."
    $safeFailed = [ordered]@{
        version = 1
        platform = $platform
        mode = $mode
        state = "failed"
        error = "p0_finalization_failed"
        phases = $phases
    }
    foreach ($surface in $surfaceNames) {
        $safeFailed[$surface] = [ordered]@{
            status = "failed"
            missing_evidence = @("late_cleanup_or_security_failure")
        }
    }
    $pendingReport = [pscustomobject]$safeFailed
    Set-LateFailure -Report $pendingReport -Code "late_cleanup_or_security_failure"
    Write-Utf8File $artifactReport ($pendingReport | ConvertTo-Json -Depth 20)
    if (Test-Path -LiteralPath $artifactPng -PathType Leaf) {
        Remove-Item -LiteralPath $artifactPng -Force
    }
    Write-Junit $artifactJunit $pendingReport $null $failure
}

if ($null -ne $failure) { throw $failure }

exit 0
