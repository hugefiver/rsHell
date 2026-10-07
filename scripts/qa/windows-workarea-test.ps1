# No P/Invoke: test imports/mocked PS boundaries first, then compile declarations only
# and exercise a private managed error seam without calling any native entry point.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script:Checks = 0
$script:NativeInitializations = 0
$script:AddTypeCalls = 0
$script:Calls = [System.Collections.Generic.List[string]]::new()

function Assert-WorkAreaTest {
    param([bool]$Condition, [string]$Name)
    if (-not $Condition) { throw "Work area boundary test failed: $Name" }
    $script:Checks++
}

function Assert-WorkAreaTestThrows {
    param([scriptblock]$Action, [string]$Name)
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    Assert-WorkAreaTest $failed $Name
}

function Add-Type {
    $script:AddTypeCalls++
    throw 'Production native initialization is forbidden in no-native work area tests.'
}

foreach ($name in @('windows-display-native.ps1', 'windows-workarea.ps1', 'windows-workarea-test.ps1')) {
    $tokens = $null
    $parseErrors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name), [ref]$tokens, [ref]$parseErrors)
    Assert-WorkAreaTest ($parseErrors.Count -eq 0) "syntax: $name"
    if ($name -ne 'windows-workarea-test.ps1') {
        Assert-WorkAreaTest (@($ast.EndBlock.Statements | Where-Object { $_ -isnot [System.Management.Automation.Language.FunctionDefinitionAst] }).Count -eq 0) "definitions-only import: $name"
    }
    if ($name -eq 'windows-display-native.ps1') {
        $nativeDeclaration = $ast.Find({
            param($node)
            $node -is [System.Management.Automation.Language.CommandAst] -and $node.GetCommandName() -ceq 'Add-Type'
        }, $true)
        $nativeSource = $nativeDeclaration.CommandElements[2].Value
    }
}
foreach ($typeName in @('RshellDisplayConfiguration', 'RshellWorkAreaConfiguration', 'RshellWorkAreaRect')) {
    Assert-WorkAreaTest ($null -eq ($typeName -as [type])) "fresh no-native process: $typeName"
}
$importOutput = @(. (Join-Path $PSScriptRoot 'windows-display-native.ps1'); . (Join-Path $PSScriptRoot 'windows-workarea.ps1'))
Assert-WorkAreaTest ($importOutput.Count -eq 0 -and $script:AddTypeCalls -eq 0) 'silent imports without Add-Type'

# Tripwire remains installed even when the three native call boundaries are replaced.
function Initialize-WorkspaceWorkAreaNative {
    $script:NativeInitializations++
    throw 'Native initialization is forbidden in no-native work area tests.'
}

$requested = [pscustomobject]@{ Left = -40; Top = -20; Right = 1920; Bottom = 1080 }
$normalized = ConvertTo-WorkspaceWorkAreaRect $requested
Assert-WorkAreaTest ($normalized.Left -eq -40 -and $normalized.Top -eq -20) 'negative coordinates accepted'
Assert-WorkAreaTest (@($normalized.PSObject.Properties).Count -eq 4) 'exact four-field result'
foreach ($field in @('Left', 'Top', 'Right', 'Bottom')) {
    Assert-WorkAreaTest ($normalized.$field -is [int]) "normalized Int32: $field"
}
$jsonRect = ConvertTo-WorkspaceWorkAreaRect ($requested | ConvertTo-Json -Compress | ConvertFrom-Json)
Assert-WorkAreaTest ($jsonRect.Right -eq 1920 -and $jsonRect.Left -is [int]) 'JSON Int64 coordinates normalized'
$limitRect = ConvertTo-WorkspaceWorkAreaRect @{ Left = [long][int]::MinValue; Top = [long][int]::MinValue; Right = [long][int]::MaxValue; Bottom = [long][int]::MaxValue }
Assert-WorkAreaTest ($limitRect.Left -eq [int]::MinValue -and $limitRect.Right -eq [int]::MaxValue) 'signed Int32 limits and wide positive extent'

foreach ($invalid in @($null, 'rectangle', 1, @(), [pscustomobject]@{},
        [pscustomobject]@{ Left = 0; Top = 0; Right = 10 },
        [pscustomobject]@{ Left = 0; Top = 0; Right = 10; Bottom = 10; Extra = 1 },
        @{ left = 0; Top = 0; Right = 10; Bottom = 10 },
        @{ Left = 0; Top = 0; Right = 0; Bottom = 10 },
        @{ Left = 10; Top = 0; Right = 0; Bottom = 10 },
        @{ Left = 0; Top = 10; Right = 10; Bottom = 10 },
        @{ Left = 0; Top = 10; Right = 10; Bottom = 0 })) {
    Assert-WorkAreaTestThrows { ConvertTo-WorkspaceWorkAreaRect $invalid } 'malformed shape or extent rejected'
}
Assert-WorkAreaTestThrows { ConvertTo-WorkspaceWorkAreaRect @{ Left = @(0); Top = 0; Right = 10; Bottom = 10 } } 'singleton array coordinate rejected'
Assert-WorkAreaTestThrows { ConvertTo-WorkspaceWorkAreaRect ([pscustomobject]@{ Left = @(0); Top = 0; Right = 10; Bottom = 10 }) } 'singleton property array coordinate rejected'
foreach ($coordinate in @($null, '0', $false, [double]0, [decimal]0, [uint32]0, [long]2147483648, [long]-2147483649)) {
    foreach ($field in @('Left', 'Top', 'Right', 'Bottom')) {
        $invalid = @{ Left = -40; Top = -20; Right = 1920; Bottom = 1080 }
        $invalid[$field] = $coordinate
        Assert-WorkAreaTestThrows { ConvertTo-WorkspaceWorkAreaRect $invalid } "invalid numeric coordinate rejected: $field"
    }
}

# Real guard, with rejected environments, must stop both setter entry points before initialization.
$actionsBefore = [Environment]::GetEnvironmentVariable('GITHUB_ACTIONS')
$runnerBefore = [Environment]::GetEnvironmentVariable('RUNNER_ENVIRONMENT')
try {
    foreach ($environment in @(
            @{ Actions = $null; Runner = $null },
            @{ Actions = 'false'; Runner = 'github-hosted' },
            @{ Actions = 'true'; Runner = 'self-hosted' },
            @{ Actions = 'TRUE'; Runner = 'github-hosted' },
            @{ Actions = 'true'; Runner = 'GITHUB-HOSTED' },
            @{ Actions = 'true'; Runner = $null },
            @{ Actions = $null; Runner = 'github-hosted' })) {
        [Environment]::SetEnvironmentVariable('GITHUB_ACTIONS', $environment.Actions)
        [Environment]::SetEnvironmentVariable('RUNNER_ENVIRONMENT', $environment.Runner)
        Assert-WorkAreaTestThrows { Set-WorkspaceWorkArea -Rect $requested } 'public setter rejects non-hosted environment'
        Assert-WorkAreaTestThrows { Set-WorkspaceWorkAreaNative -Rect $requested } 'native boundary rejects non-hosted environment'
        Assert-WorkAreaTest ($script:NativeInitializations -eq 0) 'hosted guard precedes native initialization'
    }
    [Environment]::SetEnvironmentVariable('GITHUB_ACTIONS', 'true')
    [Environment]::SetEnvironmentVariable('RUNNER_ENVIRONMENT', 'github-hosted')
    # Checking the guard alone does not initialize or invoke native code.
    if ($IsWindows) {
        Assert-WorkspaceWorkAreaHostedRunner
        Assert-WorkAreaTest $true 'hosted Windows guard accepts exact environment'
    }
    else {
        Assert-WorkAreaTestThrows { Assert-WorkspaceWorkAreaHostedRunner } 'non-Windows platform rejected even with hosted environment'
    }
}
finally {
    [Environment]::SetEnvironmentVariable('GITHUB_ACTIONS', $actionsBefore)
    [Environment]::SetEnvironmentVariable('RUNNER_ENVIRONMENT', $runnerBefore)
}

# Positive setter paths use only replaceable PowerShell functions, never the managed/native API.
function Assert-WorkspaceWorkAreaHostedRunner { $script:Calls.Add('guard') }
$script:Actual = [pscustomobject]@{ Left = -40; Top = -20; Right = 1918; Bottom = 1038 }
$script:Primary = [pscustomobject]@{ Left = -1920; Top = 120; Right = 0; Bottom = 1200 }
$script:SetFailure = $false
$script:QueryFailure = $false
$script:SetRect = $null

function Get-WorkspaceWorkAreaNative {
    $script:Calls.Add('get')
    if ($script:QueryFailure) { throw 'Simulated work area query failure.' }
    return $script:Actual
}

function Get-WorkspacePrimaryMonitorRectNative {
    $script:Calls.Add('primary')
    return $script:Primary
}

function Set-WorkspaceWorkAreaNative {
    param($Rect)
    $script:Calls.Add('set')
    $script:SetRect = $Rect
    if ($script:SetFailure) { throw 'Simulated SPI_SETWORKAREA rejection.' }
    return $Rect # Even an accidental boundary echo must not become the public result.
}

$current = Get-WorkspaceWorkArea
Assert-WorkAreaTest ($current.Right -eq 1918 -and $current.Bottom -eq 1038) 'GET returns queried rectangle'
$primary = Get-WorkspacePrimaryMonitorRect
Assert-WorkAreaTest ($primary.Left -eq -1920 -and $primary.Top -eq 120 -and $primary.Bottom -eq 1200) 'primary query preserves nonzero origin'
$script:Calls.Clear()
$actual = Set-WorkspaceWorkArea -Rect $requested
Assert-WorkAreaTest (($script:Calls -join ',') -ceq 'guard,set,get') 'guard then SET then independent GET'
Assert-WorkAreaTest ($script:SetRect.Right -eq 1920 -and $script:SetRect.Left -is [int]) 'setter receives validated requested coordinates'
Assert-WorkAreaTest ($actual.Right -eq 1918 -and $actual.Bottom -eq 1038) 'nonconverged actual returned instead of fake target'

$script:Calls.Clear()
Assert-WorkAreaTestThrows { Set-WorkspaceWorkArea -Rect @{ Left = '0'; Top = 0; Right = 10; Bottom = 10 } } 'invalid setter input rejected'
Assert-WorkAreaTest (($script:Calls -join ',') -ceq 'guard') 'invalid rectangle never reaches native boundary'
$script:Calls.Clear()
$script:SetFailure = $true
Assert-WorkAreaTestThrows { Set-WorkspaceWorkArea -Rect $requested } 'SET error propagated'
Assert-WorkAreaTest (($script:Calls -join ',') -ceq 'guard,set') 'failed SET does not fabricate readback'
$script:SetFailure = $false
$script:QueryFailure = $true
$script:Calls.Clear()
Assert-WorkAreaTestThrows { Set-WorkspaceWorkArea -Rect $requested } 'post-SET GET error propagated'
Assert-WorkAreaTest (($script:Calls -join ',') -ceq 'guard,set,get') 'post-SET query attempted exactly once'
$script:QueryFailure = $false
$script:Actual = [pscustomobject]@{ Left = 0; Top = 0; Right = 0; Bottom = 10 }
Assert-WorkAreaTestThrows { Get-WorkspaceWorkArea } 'malformed native GET result rejected'
Assert-WorkAreaTestThrows { Set-WorkspaceWorkArea -Rect $requested } 'malformed post-SET GET result rejected'
$script:Primary = [pscustomobject]@{ Left = 0; Top = 0; Right = 10; Bottom = 10; Extra = 1 }
Assert-WorkAreaTestThrows { Get-WorkspacePrimaryMonitorRect } 'malformed primary query result rejected'

Assert-WorkAreaTest ($script:NativeInitializations -eq 0 -and $script:AddTypeCalls -eq 0) 'no native initialization throughout tests'
foreach ($typeName in @('RshellDisplayConfiguration', 'RshellWorkAreaConfiguration', 'RshellWorkAreaRect')) {
    Assert-WorkAreaTest ($null -eq ($typeName -as [type])) "native type still unloaded: $typeName"
}
[Console]::WriteLine("RSHELL_WORKAREA_BOUNDARY_TEST checks=$script:Checks native_initialized=0 native_calls=0")

# Compile the exact production declarations, not the initializer. DllImport declarations
# alone do not load/call user32; only the pure private exception seam below is invoked.
$managedChecksBefore = $script:Checks
Microsoft.PowerShell.Utility\Add-Type -TypeDefinition $nativeSource
$managedType = 'RshellWorkAreaConfiguration' -as [type]
Assert-WorkAreaTest ($null -ne $managedType) 'production managed declarations compile'
foreach ($name in @('GetPrimaryMonitorRect', 'SetWorkArea')) {
    $methodBody = $managedType.GetMethod($name).GetMethodBody()
    $catches = @($methodBody.ExceptionHandlingClauses | Where-Object { $_.Flags -eq [System.Reflection.ExceptionHandlingClauseOptions]::Clause -and $_.CatchType -eq [Exception] })
    Assert-WorkAreaTest ($catches.Count -eq 2) "$name captures operation error and protects throwing DPI cleanup"
}
$privateStatic = [System.Reflection.BindingFlags]::NonPublic -bor [System.Reflection.BindingFlags]::Static
$throwDpiFailure = $managedType.GetMethod('ThrowDpiRestoreFailure', $privateStatic)
Assert-WorkAreaTest ($null -ne $throwDpiFailure -and $throwDpiFailure.IsPrivate) 'error aggregation seam stays private'
$callBytes = '28-' + [BitConverter]::ToString([BitConverter]::GetBytes($throwDpiFailure.MetadataToken))
foreach ($name in @('GetPrimaryMonitorRect', 'SetWorkArea')) {
    $il = [BitConverter]::ToString($managedType.GetMethod($name).GetMethodBody().GetILAsByteArray())
    Assert-WorkAreaTest ($il.Contains($callBytes)) "$name calls the production aggregation seam"
}

function Get-ManagedDpiTestFailure {
    param([Exception]$OperationError, [Exception]$RestoreError)
    try { [void]$throwDpiFailure.Invoke($null, [object[]]@($OperationError, $RestoreError)) }
    catch {
        $failure = $_.Exception
        while ($failure -is [System.Management.Automation.MethodInvocationException] -or $failure -is [System.Reflection.TargetInvocationException]) {
            $failure = $failure.InnerException
        }
        return $failure
    }
    throw 'The managed DPI error seam did not throw.'
}

$restoreError = [System.ComponentModel.Win32Exception]::new(50, 'Fixture DPI restore failure.')
foreach ($operationError in @(
        [System.ComponentModel.Win32Exception]::new(5, 'Fixture native query failure.'),
        [ArgumentException]::new('Fixture rectangle validation failure.'),
        [System.ComponentModel.Win32Exception]::new(87, 'Fixture native set failure.'))) {
    $failure = Get-ManagedDpiTestFailure $operationError $restoreError
    Assert-WorkAreaTest ($failure -is [AggregateException]) 'operation plus cleanup throws AggregateException'
    Assert-WorkAreaTest ($failure.InnerExceptions.Count -eq 2) 'both errors retained without substitution'
    Assert-WorkAreaTest ([object]::ReferenceEquals($failure.InnerExceptions[0], $operationError)) 'original error instance retained first'
    Assert-WorkAreaTest ([object]::ReferenceEquals($failure.InnerExceptions[1], $restoreError)) 'cleanup error instance retained second'
}
$failure = Get-ManagedDpiTestFailure $null $restoreError
Assert-WorkAreaTest ([object]::ReferenceEquals($failure, $restoreError)) 'cleanup-only failure retains original exception'
Assert-WorkAreaTest ($script:NativeInitializations -eq 0 -and $script:AddTypeCalls -eq 0) 'managed regression never invokes production initialization'
[Console]::WriteLine("RSHELL_WORKAREA_MANAGED_TEST checks=$($script:Checks - $managedChecksBefore) declarations_compiled=1 native_calls=0")
