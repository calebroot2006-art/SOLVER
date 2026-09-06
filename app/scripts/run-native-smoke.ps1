$ErrorActionPreference = 'Stop'
# Account creation is restricted to disposable GitHub-hosted Windows machines.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or
    $env:RUNNER_OS -ne 'Windows' -or -not $env:RUNNER_TEMP) {
    throw 'This launcher runs only on a disposable GitHub-hosted Windows runner.'
}

$appDirectory = Split-Path -Parent $PSScriptRoot
$resultsDirectory = Join-Path $appDirectory 'test-results'
$nodeBinary = (Get-Command node.exe -ErrorAction Stop).Source
$runnerIdentity = Get-Content -LiteralPath (Join-Path $resultsDirectory 'runner-identity.json') -Raw | ConvertFrom-Json
if (-not $runnerIdentity.highIntegrity -and -not $runnerIdentity.systemIntegrity -and -not $runnerIdentity.administrator) {
    & $nodeBinary (Join-Path $PSScriptRoot 'native-smoke.mjs')
    exit $LASTEXITCODE
}

$testAccountName = 'gto-ci-' + [guid]::NewGuid().ToString('N').Substring(0, 10)
$stageDirectory = Join-Path $env:RUNNER_TEMP ('gto-native-' + [guid]::NewGuid().ToString('N'))
$resolvedRunnerTemp = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
$resolvedStage = [IO.Path]::GetFullPath($stageDirectory)
if (-not $resolvedStage.StartsWith($resolvedRunnerTemp, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The test staging directory must be inside RUNNER_TEMP.'
}
$stagedApp = Join-Path $stageDirectory 'app'
$stagedScripts = Join-Path $stagedApp 'scripts'
$stagedResults = Join-Path $stagedApp 'test-results'
$stagedRelease = Join-Path $stagedApp 'src-tauri/target/release'
$stagedDriver = Join-Path $stageDirectory 'msedgedriver.exe'
$testUser = $null
$probeProcess = $null
$launchEvidence = @{ standardAccount = $testAccountName; passed = $false }
$resultCode = 1

try {
    foreach ($directory in @($stagedScripts, $stagedResults, $stagedRelease, (Join-Path $stageDirectory 'profile'))) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }
    Get-ChildItem -LiteralPath $PSScriptRoot -File | Copy-Item -Destination $stagedScripts
    Copy-Item -LiteralPath (Join-Path $appDirectory 'src-tauri/target/release/app.exe') -Destination $stagedRelease
    Copy-Item -LiteralPath $env:TAURI_TEST_EDGE_DRIVER -Destination $stagedDriver
    Copy-Item -LiteralPath (Join-Path $resultsDirectory 'webdriver-environment.json') -Destination $stagedResults
    $originalHash = (Get-FileHash -LiteralPath (Join-Path $appDirectory 'src-tauri/target/release/app.exe') -Algorithm SHA256).Hash
    $stagedHash = (Get-FileHash -LiteralPath (Join-Path $stagedRelease 'app.exe') -Algorithm SHA256).Hash
    if ($stagedHash -ne $originalHash) { throw 'The staged release executable differs from the build output.' }
    if ((Get-FileHash -LiteralPath $stagedDriver -Algorithm SHA256).Hash -ne
        (Get-FileHash -LiteralPath $env:TAURI_TEST_EDGE_DRIVER -Algorithm SHA256).Hash) {
        throw 'The staged driver differs from the signature-checked driver.'
    }
    $launchEvidence.binarySha256 = $stagedHash

    # This generated credential exists only in this process; it is never logged or persisted.
    $passwordBytes = [Security.Cryptography.RandomNumberGenerator]::GetBytes(32)
    $passwordText = [Convert]::ToBase64String($passwordBytes) + '!aA1'
    $securePassword = ConvertTo-SecureString $passwordText -AsPlainText -Force
    $passwordText = $null
    $testUser = New-LocalUser -Name $testAccountName -Password $securePassword -Description 'Disposable GTO native CI probe'
    if (-not (Get-LocalGroupMember -SID 'S-1-5-32-545' | Where-Object { $_.SID -eq $testUser.SID })) {
        Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $testUser
    }
    $credential = [PSCredential]::new("$env:COMPUTERNAME\$testAccountName", $securePassword)

    # Grant only this new account access to the disposable staged test directory.
    $stageAcl = Get-Acl -LiteralPath $stageDirectory
    $stageAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
        $testUser.SID, 'Modify', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
    Set-Acl -LiteralPath $stageDirectory -AclObject $stageAcl

    # The credential launch path may discard Start-Process environment overrides.
    # Pass only these non-secret test inputs through the account's staging folder.
    @{
        commit = $env:GITHUB_SHA
        nativeDriver = $stagedDriver
        webviewFolder = $env:TAURI_TEST_WEBVIEW_FOLDER
        userDataFolder = (Join-Path $stageDirectory 'profile')
        temporaryDirectory = $stageDirectory
    } | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $stagedResults 'native-launch.json') -Encoding utf8

    $startOptions = @{
        FilePath = $nodeBinary
        ArgumentList = '"' + (Join-Path $stagedScripts 'native-smoke.mjs') + '"'
        WorkingDirectory = $stagedApp
        Credential = $credential
        LoadUserProfile = $true
        WindowStyle = 'Hidden'
        PassThru = $true
        UseNewEnvironment = $true
        RedirectStandardOutput = (Join-Path $resultsDirectory 'standard-user-stdout.log')
        RedirectStandardError = (Join-Path $resultsDirectory 'standard-user-stderr.log')
    }
    $probeProcess = Start-Process @startOptions
    $launchEvidence.probePid = $probeProcess.Id
    if (-not $probeProcess.WaitForExit(240000)) { throw 'The standard-user native probe exceeded four minutes.' }
    if ($null -eq $probeProcess.ExitCode) { throw 'The native probe did not supply an exit code.' }
    $resultCode = $probeProcess.ExitCode
    $launchEvidence.passed = $resultCode -eq 0
} catch {
    $launchEvidence.error = $_.Exception.ToString()
    Write-Error -Message $_ -ErrorAction Continue
} finally {
    $cleanupErrors = [System.Collections.Generic.List[string]]::new()
    # Cleanup operations are independent: a file-copy failure must not skip account removal.
    $cleanupActions = @(
        {
            # The account is unique to this test. Stop only processes owned by its SID.
            if ($testUser) {
                foreach ($candidate in @(Get-CimInstance Win32_Process)) {
                    $owner = Invoke-CimMethod -InputObject $candidate -MethodName GetOwnerSid -ErrorAction SilentlyContinue
                    if ($owner.Sid -eq $testUser.SID.Value) {
                        Stop-Process -Id $candidate.ProcessId -Force -ErrorAction SilentlyContinue
                    }
                }
            }
        },
        {
            if (Test-Path -LiteralPath $stagedResults) {
                Get-ChildItem -LiteralPath $stagedResults -File | Copy-Item -Destination $resultsDirectory -Force
            }
        },
        {
            if (-not $testUser) { return }
            $testProfile = Get-CimInstance Win32_UserProfile -Filter "SID='$($testUser.SID.Value)'"
            if ($testProfile) {
                $profileParent = [IO.Path]::GetFullPath((Split-Path -Parent $env:PUBLIC)).TrimEnd('\') + '\'
                $profilePath = [IO.Path]::GetFullPath($testProfile.LocalPath)
                if (-not $profilePath.StartsWith($profileParent, [StringComparison]::OrdinalIgnoreCase) -or
                    (Split-Path -Leaf $profilePath) -notlike "$testAccountName*") {
                    throw 'Refusing to remove a profile outside the new test account directory.'
                }
                $testProfile | Remove-CimInstance -ErrorAction Stop
            }
        },
        {
            if ($testUser) {
                Remove-LocalUser -SID $testUser.SID -ErrorAction Stop
                $launchEvidence.accountRemoved = $true
            }
        },
        {
            # Recheck the exact absolute target before recursive cleanup.
            if ([IO.Path]::GetFullPath($stageDirectory) -ne $resolvedStage -or
                -not $resolvedStage.StartsWith($resolvedRunnerTemp, [StringComparison]::OrdinalIgnoreCase)) {
                throw 'Refusing cleanup outside the verified test staging directory.'
            }
            if (Test-Path -LiteralPath $stageDirectory) {
                Remove-Item -LiteralPath $stageDirectory -Recurse -Force
            }
        }
    )
    foreach ($cleanupAction in $cleanupActions) {
        try { & $cleanupAction } catch { $cleanupErrors.Add($_.Exception.ToString()) }
    }
    if ($cleanupErrors.Count -gt 0) {
        $launchEvidence.cleanupErrors = @($cleanupErrors)
        $launchEvidence.passed = $false
        $resultCode = 1
    }
    $launchEvidence | ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath (Join-Path $resultsDirectory 'standard-user-launch.json') -Encoding utf8
}
exit $resultCode
