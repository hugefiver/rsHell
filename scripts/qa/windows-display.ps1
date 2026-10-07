param(
    [Parameter(Mandatory)][ValidateSet("Apply", "Restore", "Probe", "ApplySelected")][string]$Mode,
    [Parameter(Mandatory)][ValidateNotNullOrEmpty()][string]$Ledger,
    [int]$Width = 2560,
    [int]$Height = 1440,
    [int]$BitsPerPixel = 0,
    [int]$Frequency = 0,
    [ValidateSet("Fullscreen", "Dynamic")][string]$ChangeKind = "Fullscreen"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "windows-display-experiment.ps1")

try {
    Initialize-WorkspaceDisplayNative
    switch ($Mode) {
        "Probe" {
            $target = [RshellDisplayConfiguration]::PreferredAtLeast($Width, $Height)
            [RshellDisplayConfiguration]::Test($target)
            Write-Output "RSHELL_DISPLAY_PROBE width=$($target.Width) height=$($target.Height) bpp=$($target.BitsPerPixel) frequency=$($target.Frequency)"
            exit 0
        }
        "Apply" {
            $baseline = Get-WorkspaceDisplayCurrent
            Write-DisplayNewFile $Ledger ($baseline | ConvertTo-Json -Compress)
            Assert-DisplayEqual (Read-DisplayBaseline $Ledger) $baseline
            $target = Select-WorkspaceDisplayTarget $Width $Height
        }
        "ApplySelected" {
            $target = ConvertTo-DisplayMode ([pscustomobject]@{
                Width = $Width; Height = $Height; BitsPerPixel = $BitsPerPixel; Frequency = $Frequency
            })
        }
        "Restore" {
            $target = Read-DisplayBaseline $Ledger
            $ChangeKind = "Dynamic"
        }
    }
    $restoreMode = [RshellDisplayMode]::new()
    $restoreMode.Width = $target.Width
    $restoreMode.Height = $target.Height
    $restoreMode.BitsPerPixel = $target.BitsPerPixel
    $restoreMode.Frequency = $target.Frequency
    [RshellDisplayConfiguration]::Test($restoreMode)
    [RshellDisplayConfiguration]::Apply($restoreMode, ($ChangeKind -eq "Fullscreen"))
    $actual = Get-WorkspaceDisplayCurrent
    Assert-DisplayEqual $actual $target
    if ($Mode -eq "ApplySelected") {
        # One finite line of actual Current, never a target echo. The supervisor labels it.
        Write-Output "RSHELL_DISPLAY_CHILD width=$($actual.Width) height=$($actual.Height) bpp=$($actual.BitsPerPixel) frequency=$($actual.Frequency)"
    }
    elseif ($Mode -eq "Apply") {
        $flags = if ($ChangeKind -eq "Fullscreen") { 4 } else { 0 }
        Write-Output "RSHELL_DISPLAY_APPLIED width=$($actual.Width) height=$($actual.Height) bpp=$($actual.BitsPerPixel) frequency=$($actual.Frequency) flags=$flags"
    }
    if ($Mode -eq "Restore") {
        Write-Output "RSHELL_DISPLAY_RESTORED width=$($actual.Width) height=$($actual.Height) bpp=$($actual.BitsPerPixel) frequency=$($actual.Frequency) flags=0"
    }
}
catch {
    [Console]::Error.WriteLine("RSHELL_DISPLAY helper_failed")
    exit 1
}
