$ErrorActionPreference = "Stop"
$check = Join-Path $PSScriptRoot "check_winget_registration.ps1"
$output = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$runner = "$output.ps1"
$pwsh = (Get-Process -Id $PID).Path
$previousExitCode = $global:LASTEXITCODE

try {
    # Match GitHub Actions' `pwsh` wrapper, including its exit-code propagation.
    @'
param([string]$Check, [string]$OutputFile, [int]$Status)
$ErrorActionPreference = "Stop"
function gh {
    $global:LASTEXITCODE = if ($Status -eq 200) { 0 } else { 1 }
    "HTTP/2.0 $Status`nContent-Type: application/json`n`n{}"
}
& $Check $OutputFile
if (Test-Path -LiteralPath variable:\LASTEXITCODE) { exit $LASTEXITCODE }
'@ | Set-Content $runner
    foreach ($status in 200, 404, 403, 500) {
        $log = & $pwsh -NoProfile -NonInteractive -File $runner $check $output $status 2>&1 | Out-String
        if ($status -in 200, 404) {
            if ($LASTEXITCODE -ne 0) { throw "registration check failed for HTTP ${status}: $log" }
            $expected = if ($status -eq 200) { "registered=true" } else { "registered=false" }
            if ((Get-Content $output -Raw).Trim() -ne $expected) { throw "unexpected output for HTTP $status" }
            Remove-Item $output
        } else {
            if ($LASTEXITCODE -eq 0) { throw "API failure was ignored for HTTP $status" }
            if (Test-Path $output) { throw "API failure produced registration output" }
        }
        Write-Output "PASS registration check: HTTP $status"
    }
} finally {
    Remove-Item $output, $runner -ErrorAction SilentlyContinue
    $global:LASTEXITCODE = $previousExitCode
}
