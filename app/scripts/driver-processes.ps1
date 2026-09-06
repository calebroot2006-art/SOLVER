param([Parameter(Mandatory = $true)][int]$DriverProcessId)
$ErrorActionPreference = 'Stop'
# Read only descendants of the test driver; exclude unrelated desktop processes.
$processes = @(Get-CimInstance Win32_Process)
$selectedIds = [System.Collections.Generic.HashSet[uint32]]::new()
$selectedIds.Add([uint32]$DriverProcessId) | Out-Null
do {
    $added = $false
    foreach ($process in $processes) {
        if ($selectedIds.Contains($process.ParentProcessId) -and $selectedIds.Add($process.ProcessId)) {
            $added = $true
        }
    }
} while ($added)
$selected = @($processes | Where-Object { $selectedIds.Contains($_.ProcessId) } |
    Select-Object Name, ProcessId, ParentProcessId, ExecutablePath, CommandLine,
        CreationDate, ExecutionState, ThreadCount, ExitCode)
ConvertTo-Json -InputObject $selected -Depth 4
