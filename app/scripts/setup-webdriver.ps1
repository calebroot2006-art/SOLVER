$ErrorActionPreference = 'Stop'
if (-not $env:RUNNER_TEMP -or -not $env:GITHUB_ENV) {
    throw 'This setup runs in hosted GitHub Actions and requires RUNNER_TEMP and GITHUB_ENV.'
}

$appDirectory = Split-Path -Parent $PSScriptRoot
$resultsDirectory = Join-Path $appDirectory 'test-results'
New-Item -ItemType Directory -Path $resultsDirectory -Force | Out-Null
$toolsDirectory = Join-Path $env:RUNNER_TEMP 'gto-webdriver-tools'
New-Item -ItemType Directory -Path $toolsDirectory -Force | Out-Null

# Read the installed runtime rather than assuming Edge and WebView2 versions agree.
$runtimeRoots = @(
    (Join-Path ${env:ProgramFiles(x86)} 'Microsoft/EdgeWebView/Application'),
    (Join-Path $env:ProgramFiles 'Microsoft/EdgeWebView/Application')
)
$runtimeDirectories = foreach ($runtimeRoot in $runtimeRoots) {
    if (Test-Path -LiteralPath $runtimeRoot) {
        Get-ChildItem -LiteralPath $runtimeRoot -Directory |
            Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' -and
                (Test-Path -LiteralPath (Join-Path $_.FullName 'msedgewebview2.exe')) }
    }
}
$runtimeDirectory = $runtimeDirectories | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $runtimeDirectory) { throw 'The Windows runner has no installed WebView2 runtime.' }
$runtimeVersion = $runtimeDirectory.Name
$requiredBuild = ($runtimeVersion.Split('.')[0..2] -join '.')

$driverCommand = Get-Command msedgedriver.exe -ErrorAction SilentlyContinue
$edgeDriver = if ($driverCommand) { $driverCommand.Source } else { $null }
if (-not $edgeDriver -and $env:EDGEWEBDRIVER) {
    $edgeDriver = Join-Path $env:EDGEWEBDRIVER 'msedgedriver.exe'
}
$downloadUrl = $null
function Assert-MicrosoftDriverSignature([string]$Path) {
    $verifiedSignature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($verifiedSignature.Status -ne 'Valid' -or $verifiedSignature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
        throw 'The Edge WebDriver binary does not have a valid Microsoft signature.'
    }
    return $verifiedSignature
}
$driverVersion = if ($edgeDriver -and (Test-Path -LiteralPath $edgeDriver)) {
    $signature = Assert-MicrosoftDriverSignature $edgeDriver
    (& $edgeDriver --version) -replace '^.*?(\d+\.\d+\.\d+\.\d+).*$', '$1'
} else { '' }
if ($driverVersion -notmatch ('^' + [regex]::Escape($requiredBuild) + '\.')) {
    $downloadUrl = "https://msedgedriver.microsoft.com/$runtimeVersion/edgedriver_win64.zip"
    $archive = Join-Path $toolsDirectory 'edgedriver_win64.zip'
    Invoke-WebRequest -Uri $downloadUrl -OutFile $archive
    Expand-Archive -LiteralPath $archive -DestinationPath $toolsDirectory -Force
    $edgeDriver = Join-Path $toolsDirectory 'msedgedriver.exe'
    $signature = Assert-MicrosoftDriverSignature $edgeDriver
    $driverVersion = (& $edgeDriver --version) -replace '^.*?(\d+\.\d+\.\d+\.\d+).*$', '$1'
}
if ($driverVersion -notmatch ('^' + [regex]::Escape($requiredBuild) + '\.')) {
    throw "Edge WebDriver $driverVersion does not match WebView2 $runtimeVersion."
}

# Install the official exact driver version in disposable runner storage. Its
# published Cargo.lock pins transitive dependencies; app dependencies do not change.
cargo install tauri-driver --version '=2.0.6' --locked --root $toolsDirectory
if ($LASTEXITCODE -ne 0) { throw 'Installing tauri-driver 2.0.6 failed.' }
$tauriDriver = Join-Path $toolsDirectory 'bin/tauri-driver.exe'
if (-not (Test-Path -LiteralPath $tauriDriver)) { throw 'tauri-driver.exe was not installed.' }

@{
    tauriDriverVersion = '2.0.6'
    tauriDriverSha256 = (Get-FileHash -LiteralPath $tauriDriver -Algorithm SHA256).Hash
    webviewVersion = $runtimeVersion
    edgeDriverVersion = $driverVersion
    edgeDriverSha256 = (Get-FileHash -LiteralPath $edgeDriver -Algorithm SHA256).Hash
    edgeDriverSigner = $signature.SignerCertificate.Subject
    edgeDriverDownloadUrl = $downloadUrl
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $resultsDirectory 'webdriver-environment.json') -Encoding utf8

"TAURI_TEST_DRIVER=$tauriDriver" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
"TAURI_TEST_EDGE_DRIVER=$edgeDriver" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
"TAURI_TEST_WEBVIEW_FOLDER=$($runtimeDirectory.FullName)" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
Write-Output "WebView2 $runtimeVersion, Microsoft EdgeDriver $driverVersion, tauri-driver 2.0.6"
