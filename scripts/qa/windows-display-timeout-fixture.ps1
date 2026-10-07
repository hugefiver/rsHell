# Fixed no-native cases only; no runtime payload/program/process parameters.
param([ValidateSet('Sleep', 'Display', 'DisplayRestore', 'Workarea', 'WorkareaRestore', 'Invalid', 'Overflow', 'ExtraLine', 'Stderr', 'Nonzero')][string]$Scenario = 'Sleep')
switch ($Scenario) {
    'Sleep' { Start-Sleep -Seconds 30 }
    'Display' { [Console]::WriteLine('RSHELL_DISPLAY_CHILD width=1920 height=1080 bpp=32 frequency=60') }
    'DisplayRestore' { [Console]::WriteLine('RSHELL_DISPLAY_RESTORED width=1024 height=768 bpp=32 frequency=60 flags=0') }
    'Workarea' { [Console]::WriteLine('RSHELL_WORKAREA_CHILD left=-120 top=30 right=1800 bottom=1110 flags=0') }
    'WorkareaRestore' { [Console]::WriteLine('RSHELL_WORKAREA_RESTORED left=-120 top=30 right=904 bottom=758 flags=0') }
    'Invalid' { [Console]::WriteLine('FIXED_INVALID_OBSERVATION') }
    'Overflow' { [Console]::WriteLine('RSHELL_WORKAREA_CHILD left=-2147483649 top=30 right=1800 bottom=1110 flags=0') }
    'ExtraLine' { [Console]::WriteLine("RSHELL_WORKAREA_CHILD left=-120 top=30 right=1800 bottom=1110 flags=0`nFIXED_EXTRA_LINE") }
    'Stderr' { [Console]::Error.WriteLine('FIXED_FIXTURE_ERROR') }
    'Nonzero' { exit 7 }
}
exit 0
