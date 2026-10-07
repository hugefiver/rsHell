# Only the fixed display helper is supervised. Tests replace its construction seam
# with a fixed, no-native sleeping fixture; no public mock/protocol switch exists.
function New-WorkspaceDisplayChildStartInfo {
    param([string]$Ledger, [string]$Mode, [string]$ChangeKind, $Target)
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = (Get-Command -Name pwsh -ErrorAction Stop).Source
    foreach ($argument in @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'windows-display.ps1'),
            '-Mode', $Mode, '-Ledger', $Ledger, '-ChangeKind', $ChangeKind,
            '-Width', [string]$Target.Width, '-Height', [string]$Target.Height,
            '-BitsPerPixel', [string]$Target.BitsPerPixel, '-Frequency', [string]$Target.Frequency)) {
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
        [ValidateRange(1, 5000)][int]$ReapMilliseconds = 5000
    )
    $marker = Join-Path $Root 'child-exit-unconfirmed'
    $process = [System.Diagnostics.Process]::new()
    $confirmed = $false
    $attempted = $false
    $started = $false
    $marked = $false
    try {
        $process.StartInfo = New-WorkspaceDisplayChildStartInfo $Ledger $Mode $ChangeKind $Target
        $process.StartInfo.UseShellExecute = $false
        $process.StartInfo.RedirectStandardOutput = $true
        $process.StartInfo.RedirectStandardError = $true
        Write-DisplayNewFile $marker '1'
        $marked = $true
        $attempted = $true
        try {
            $started = $process.Start()
            if (-not $started) { $confirmed = $true; throw "Display child did not start." }
        }
        catch [System.ComponentModel.Win32Exception] {
            # OS creation failed: no child exists. Other start errors remain uncertain.
            $confirmed = $true
            throw "Display child did not start."
        }
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
        $prefix = if ($Mode -eq 'Restore') { 'RSHELL_DISPLAY_RESTORED' } else { 'RSHELL_DISPLAY_CHILD' }
        $suffix = if ($Mode -eq 'Restore') { ' flags=0' } else { '' }
        $match = [regex]::Match($stdout.Result, "\A$prefix width=([0-9]{1,10}) height=([0-9]{1,10}) bpp=([0-9]{1,10}) frequency=([0-9]{1,10})$suffix\r?\n\z")
        if (-not $match.Success) { throw "Display child observation is invalid." }
        return ConvertTo-DisplayMode ([pscustomobject]@{
            Width = [int]$match.Groups[1].Value; Height = [int]$match.Groups[2].Value
            BitsPerPixel = [int]$match.Groups[3].Value; Frequency = [int]$match.Groups[4].Value
        })
    }
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
                [System.IO.File]::Delete($marker)
                if (Test-Path -LiteralPath $marker) { throw "Display child exit marker retained." }
            }
        }
        finally { $process.Dispose() }
    }
}
