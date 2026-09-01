param(
    [Parameter(Mandatory)]
    [ValidateSet("Apply", "Restore", "Probe")]
    [string]$Mode,

    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string]$Ledger,

    [int]$Width = 2560,
    [int]$Height = 1440
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not $IsWindows) {
    throw "Windows display configuration is unavailable on this platform."
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public sealed class RshellDisplayMode {
    public int Width { get; set; }
    public int Height { get; set; }
    public int BitsPerPixel { get; set; }
    public int Frequency { get; set; }
}

public static class RshellDisplayConfiguration {
    private const int ENUM_CURRENT_SETTINGS = -1;
    private const int DISP_CHANGE_SUCCESSFUL = 0;
    private const int CDS_TEST = 0x00000002;
    private const int CDS_FULLSCREEN = 0x00000004;
    private const int DM_BITSPERPEL = 0x00040000;
    private const int DM_PELSWIDTH = 0x00080000;
    private const int DM_PELSHEIGHT = 0x00100000;
    private const int DM_DISPLAYFREQUENCY = 0x00400000;

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
    private struct DEVMODE {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public short dmSpecVersion;
        public short dmDriverVersion;
        public short dmSize;
        public short dmDriverExtra;
        public int dmFields;
        public int dmPositionX;
        public int dmPositionY;
        public int dmDisplayOrientation;
        public int dmDisplayFixedOutput;
        public short dmColor;
        public short dmDuplex;
        public short dmYResolution;
        public short dmTTOption;
        public short dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public short dmLogPixels;
        public int dmBitsPerPel;
        public int dmPelsWidth;
        public int dmPelsHeight;
        public int dmDisplayFlags;
        public int dmDisplayFrequency;
        public int dmICMMethod;
        public int dmICMIntent;
        public int dmMediaType;
        public int dmDitherType;
        public int dmReserved1;
        public int dmReserved2;
        public int dmPanningWidth;
        public int dmPanningHeight;
    }

    [DllImport("user32.dll", CharSet = CharSet.Ansi)]
    private static extern bool EnumDisplaySettings(string deviceName, int modeNum, ref DEVMODE devMode);

    [DllImport("user32.dll", CharSet = CharSet.Ansi)]
    private static extern int ChangeDisplaySettings(ref DEVMODE devMode, int flags);

    public static RshellDisplayMode Current() {
        var mode = NewMode();
        if (!EnumDisplaySettings(null, ENUM_CURRENT_SETTINGS, ref mode)) {
            throw new InvalidOperationException("The current display mode is unavailable.");
        }
        return ToInfo(mode);
    }

    public static RshellDisplayMode Preferred(int width, int height) {
        DEVMODE? preferred = null;
        for (var index = 0; ; index++) {
            var candidate = NewMode();
            if (!EnumDisplaySettings(null, index, ref candidate)) break;
            if (candidate.dmPelsWidth != width || candidate.dmPelsHeight != height) continue;
            if (!preferred.HasValue || Score(candidate) > Score(preferred.Value)) preferred = candidate;
        }
        if (!preferred.HasValue) throw new InvalidOperationException("The required display mode is unavailable.");
        return ToInfo(preferred.Value);
    }

    public static void Test(RshellDisplayMode info) {
        var mode = FindExact(info);
        if (ChangeDisplaySettings(ref mode, CDS_TEST) != DISP_CHANGE_SUCCESSFUL) {
            throw new InvalidOperationException("The display mode test failed.");
        }
    }

    public static void Apply(RshellDisplayMode info) {
        var mode = FindExact(info);
        if (ChangeDisplaySettings(ref mode, CDS_FULLSCREEN) != DISP_CHANGE_SUCCESSFUL) {
            throw new InvalidOperationException("The display mode change failed.");
        }
        var current = Current();
        if (current.Width != info.Width || current.Height != info.Height) {
            throw new InvalidOperationException("The display mode did not converge.");
        }
    }

    private static DEVMODE FindExact(RshellDisplayMode info) {
        for (var index = 0; ; index++) {
            var candidate = NewMode();
            if (!EnumDisplaySettings(null, index, ref candidate)) break;
            if (candidate.dmPelsWidth == info.Width && candidate.dmPelsHeight == info.Height &&
                candidate.dmBitsPerPel == info.BitsPerPixel && candidate.dmDisplayFrequency == info.Frequency) {
                candidate.dmFields = DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
                return candidate;
            }
        }
        throw new InvalidOperationException("The exact display mode is unavailable.");
    }

    private static DEVMODE NewMode() {
        var mode = new DEVMODE();
        mode.dmSize = (short)Marshal.SizeOf<DEVMODE>();
        return mode;
    }

    private static int Score(DEVMODE mode) {
        var frequency = mode.dmDisplayFrequency == 60 ? 10000 : mode.dmDisplayFrequency;
        return mode.dmBitsPerPel * 100000 + frequency;
    }

    private static RshellDisplayMode ToInfo(DEVMODE mode) {
        return new RshellDisplayMode {
            Width = mode.dmPelsWidth,
            Height = mode.dmPelsHeight,
            BitsPerPixel = mode.dmBitsPerPel,
            Frequency = mode.dmDisplayFrequency,
        };
    }
}
'@

switch ($Mode) {
    "Probe" {
        $target = [RshellDisplayConfiguration]::Preferred($Width, $Height)
        [RshellDisplayConfiguration]::Test($target)
        Write-Output "RSHELL_DISPLAY_PROBE width=$($target.Width) height=$($target.Height)"
    }
    "Apply" {
        $parent = Split-Path -Parent $Ledger
        if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
            throw "The display ledger parent is unavailable."
        }
        if (Test-Path -LiteralPath $Ledger) {
            throw "The display ledger already exists."
        }
        $current = [RshellDisplayConfiguration]::Current()
        [System.IO.File]::WriteAllText(
            $Ledger,
            ($current | ConvertTo-Json -Compress),
            [System.Text.UTF8Encoding]::new($false)
        )
        $target = [RshellDisplayConfiguration]::Preferred($Width, $Height)
        [RshellDisplayConfiguration]::Test($target)
        [RshellDisplayConfiguration]::Apply($target)
        Write-Output "RSHELL_DISPLAY_APPLIED width=$($target.Width) height=$($target.Height)"
    }
    "Restore" {
        if (-not (Test-Path -LiteralPath $Ledger -PathType Leaf)) {
            throw "The display ledger is unavailable."
        }
        $saved = Get-Content -LiteralPath $Ledger -Raw | ConvertFrom-Json
        $mode = [RshellDisplayMode]::new()
        $mode.Width = [int]$saved.Width
        $mode.Height = [int]$saved.Height
        $mode.BitsPerPixel = [int]$saved.BitsPerPixel
        $mode.Frequency = [int]$saved.Frequency
        [RshellDisplayConfiguration]::Test($mode)
        [RshellDisplayConfiguration]::Apply($mode)
        Write-Output "RSHELL_DISPLAY_RESTORED width=$($mode.Width) height=$($mode.Height)"
    }
}
