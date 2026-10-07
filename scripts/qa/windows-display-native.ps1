# Importing this private helper does not initialize or call native code.
function Initialize-RshellDisplayNative {
    if (-not $IsWindows) { throw "Windows display configuration is unavailable on this platform." }
    if ('RshellDisplayConfiguration' -as [type]) { return }
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.ExceptionServices;
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

    public static RshellDisplayMode PreferredAtLeast(int width, int height) {
        DEVMODE? preferred = null;
        var available = new SortedSet<(int Width, int Height)>();
        for (var index = 0; ; index++) {
            var candidate = NewMode();
            if (!EnumDisplaySettings(null, index, ref candidate)) break;
            available.Add((candidate.dmPelsWidth, candidate.dmPelsHeight));
            if (candidate.dmPelsWidth < width || candidate.dmPelsHeight < height) continue;
            if (!preferred.HasValue || Better(candidate, preferred.Value)) preferred = candidate;
        }
        if (!preferred.HasValue) {
            var sample = new List<string>();
            foreach (var size in available) {
                if (sample.Count == 64) break;
                sample.Add($"{size.Width}x{size.Height}");
            }
            throw new InvalidOperationException(
                $"The required display mode is unavailable: requested={width}x{height} " +
                $"available_count={available.Count} available_sample=[{string.Join(",", sample)}] " +
                $"truncated_count={available.Count - sample.Count}.");
        }
        return ToInfo(preferred.Value);
    }

    public static void Test(RshellDisplayMode info) {
        var mode = FindExact(info);
        if (ChangeDisplaySettings(ref mode, CDS_TEST) != DISP_CHANGE_SUCCESSFUL) {
            throw new InvalidOperationException("The display mode test failed.");
        }
    }

    public static void Apply(RshellDisplayMode info, bool fullscreen) {
        var mode = FindExact(info);
        if (ChangeDisplaySettings(ref mode, fullscreen ? CDS_FULLSCREEN : 0) != DISP_CHANGE_SUCCESSFUL) {
            throw new InvalidOperationException("The display mode change failed.");
        }
        var current = Current();
        if (current.Width != info.Width || current.Height != info.Height ||
            current.BitsPerPixel != info.BitsPerPixel || current.Frequency != info.Frequency) {
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

    private static bool Better(DEVMODE candidate, DEVMODE current) {
        var candidateArea = (long)candidate.dmPelsWidth * candidate.dmPelsHeight;
        var currentArea = (long)current.dmPelsWidth * current.dmPelsHeight;
        if (candidateArea != currentArea) return candidateArea < currentArea;
        var candidateAt60 = candidate.dmDisplayFrequency == 60;
        var currentAt60 = current.dmDisplayFrequency == 60;
        if (candidateAt60 != currentAt60) return candidateAt60;
        if (candidate.dmBitsPerPel != current.dmBitsPerPel) return candidate.dmBitsPerPel > current.dmBitsPerPel;
        return candidate.dmDisplayFrequency < current.dmDisplayFrequency;
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

public sealed class RshellWorkAreaRect {
    public int Left { get; set; }
    public int Top { get; set; }
    public int Right { get; set; }
    public int Bottom { get; set; }
}

public static class RshellWorkAreaConfiguration {
    private const uint SPI_GETWORKAREA = 0x0030;
    private const uint SPI_SETWORKAREA = 0x002F;
    private const uint MONITOR_DEFAULTTOPRIMARY = 1;
    private const uint MONITORINFOF_PRIMARY = 1;
    private static readonly IntPtr PER_MONITOR_AWARE_V2 = new IntPtr(-4);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MONITORINFO {
        public uint cbSize;
        public RECT rcMonitor;
        public RECT rcWork;
        public uint dwFlags;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SystemParametersInfoW(uint action, uint uiParam, ref RECT rect, uint flags);

    [DllImport("user32.dll", ExactSpelling = true)]
    private static extern IntPtr MonitorFromPoint(POINT point, uint flags);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetMonitorInfoW(IntPtr monitor, ref MONITORINFO info);

    [DllImport("user32.dll", ExactSpelling = true, SetLastError = true)]
    private static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);

    public static RshellWorkAreaRect GetWorkArea() {
        // SPI_GETWORKAREA always reports physical pixels, regardless of caller DPI awareness.
        var rect = new RECT();
        if (!SystemParametersInfoW(SPI_GETWORKAREA, 0, ref rect, 0)) {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "The primary work area query failed.");
        }
        return ToInfo(rect);
    }

    public static RshellWorkAreaRect GetPrimaryMonitorRect() {
        var previous = UsePhysicalCoordinates();
        Exception operationError = null;
        try {
            var monitor = MonitorFromPoint(new POINT { X = 0, Y = 0 }, MONITOR_DEFAULTTOPRIMARY);
            if (monitor == IntPtr.Zero) {
                throw new InvalidOperationException("The primary monitor is unavailable.");
            }
            var info = new MONITORINFO { cbSize = (uint)Marshal.SizeOf<MONITORINFO>() };
            if (!GetMonitorInfoW(monitor, ref info)) {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "The primary monitor query failed.");
            }
            if ((info.dwFlags & MONITORINFOF_PRIMARY) == 0) {
                throw new InvalidOperationException("The queried monitor is not primary.");
            }
            return ToInfo(info.rcMonitor);
        }
        catch (Exception error) {
            operationError = error;
            throw;
        }
        finally {
            try { RestoreDpiContext(previous); }
            catch (Exception restoreError) { ThrowDpiRestoreFailure(operationError, restoreError); }
        }
    }

    public static void SetWorkArea(RshellWorkAreaRect info) {
        // Also protect direct managed callers, before any P/Invoke (including DPI awareness).
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows) ||
            !string.Equals(Environment.GetEnvironmentVariable("GITHUB_ACTIONS"), "true", StringComparison.Ordinal) ||
            !string.Equals(Environment.GetEnvironmentVariable("RUNNER_ENVIRONMENT"), "github-hosted", StringComparison.Ordinal)) {
            throw new InvalidOperationException("Work area changes require a GitHub-hosted Windows runner.");
        }
        if (info == null) { throw new ArgumentNullException(nameof(info)); }
        var rect = new RECT { Left = info.Left, Top = info.Top, Right = info.Right, Bottom = info.Bottom };
        Validate(rect);
        var previous = UsePhysicalCoordinates();
        Exception operationError = null;
        try {
            // Fixed zero parameters: no user-profile/registry persistence and no broadcast.
            if (!SystemParametersInfoW(SPI_SETWORKAREA, 0, ref rect, 0)) {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "The work area change failed.");
            }
        }
        catch (Exception error) {
            operationError = error;
            throw;
        }
        finally {
            try { RestoreDpiContext(previous); }
            catch (Exception restoreError) { ThrowDpiRestoreFailure(operationError, restoreError); }
        }
    }

    private static void ThrowDpiRestoreFailure(Exception operationError, Exception restoreError) {
        if (operationError != null) {
            throw new AggregateException("The work area operation and thread DPI awareness restore both failed.", operationError, restoreError);
        }
        ExceptionDispatchInfo.Capture(restoreError).Throw();
    }

    private static IntPtr UsePhysicalCoordinates() {
        var previous = SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2);
        if (previous == IntPtr.Zero) {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "Per-monitor-v2 DPI awareness is unavailable.");
        }
        return previous;
    }

    private static void RestoreDpiContext(IntPtr previous) {
        if (SetThreadDpiAwarenessContext(previous) == IntPtr.Zero) {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "The thread DPI awareness restore failed.");
        }
    }

    private static void Validate(RECT rect) {
        if (rect.Right <= rect.Left || rect.Bottom <= rect.Top) {
            throw new ArgumentException("The work area rectangle must have positive extent.");
        }
    }

    private static RshellWorkAreaRect ToInfo(RECT rect) {
        Validate(rect);
        return new RshellWorkAreaRect {
            Left = rect.Left, Top = rect.Top, Right = rect.Right, Bottom = rect.Bottom,
        };
    }
}
'@
}
