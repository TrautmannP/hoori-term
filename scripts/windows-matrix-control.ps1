param([Parameter(Mandatory)][string]$EvidenceRoot)

$ErrorActionPreference = 'Stop'
$script:MatrixRoot = (Resolve-Path -LiteralPath $EvidenceRoot).Path
$script:MatrixStep = if (Test-Path -LiteralPath "$script:MatrixRoot\step.json") {
    [long](Get-Content -LiteralPath "$script:MatrixRoot\step.json" -Raw | ConvertFrom-Json).id
} else { 0 }

function Send-MatrixStep([hashtable]$Action) {
    $script:MatrixStep++
    $Action.id = $script:MatrixStep
    $inputPath = Join-Path $script:MatrixRoot 'step.json'
    $temporaryPath = Join-Path $script:MatrixRoot 'step-input.tmp'
    [IO.File]::WriteAllText($temporaryPath, ($Action | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporaryPath -Destination $inputPath -Force
    $replyPath = Join-Path $script:MatrixRoot "step-$script:MatrixStep.json"
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        if (Test-Path -LiteralPath $replyPath) {
            try { return Get-Content -LiteralPath $replyPath -Raw | ConvertFrom-Json }
            catch { if ([DateTime]::UtcNow -ge $deadline) { throw } }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Native fixture did not acknowledge step $script:MatrixStep"
}

function Wait-Matrix([scriptblock]$Condition, [string]$Description, [int]$Seconds = 20) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        $snapshot = Send-MatrixStep @{ action = 'observe' }
        if (& $Condition $snapshot) { return $snapshot }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Matrix expectation failed: $Description (last step $script:MatrixStep)"
}

function Send-MatrixText([string]$Text, [switch]$Enter) {
    $null = Send-MatrixStep @{ action = 'text'; text = $Text }
    if ($Enter) { $null = Send-MatrixStep @{ action = 'key'; key = 'Enter' } }
}

function Send-MatrixKey([string]$Key, [switch]$Ctrl, [switch]$Shift, [switch]$Alt) {
    Send-MatrixStep @{ action = 'key'; key = $Key; ctrl = [bool]$Ctrl; shift = [bool]$Shift; alt = [bool]$Alt }
}

function Get-MatrixTab($Snapshot, [long]$Tab) {
    $Snapshot.tabs | Where-Object id -EQ $Tab
}

function Test-MatrixText($Snapshot, [long]$Tab, [string]$Text) {
    $value = Get-MatrixTab $Snapshot $Tab
    $null -ne $value -and ($value.screen -join "`n").Contains($Text)
}
