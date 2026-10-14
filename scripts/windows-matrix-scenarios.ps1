param(
    [Parameter(Mandatory)][string]$EvidenceRoot,
    [Parameter(Mandatory)][ValidateSet('row-start','row-vim','row-finish','mixed-start','mixed-shortcuts','mixed-load','mixed-close','errors')][string]$Phase,
    [ValidateRange(0,4)][int]$Row = 0
)
. "$PSScriptRoot\windows-matrix-control.ps1" -EvidenceRoot $EvidenceRoot

function Save-Checkpoint([string]$Name, $Snapshot) {
    $Snapshot | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath (Join-Path $EvidenceRoot "$Name.json") -Encoding utf8
    Write-Output "PASS $Name · step $($Snapshot.step) · $($Snapshot.title)"
}
function Assert-Matrix([bool]$Condition, [string]$Description) {
    if (-not $Condition) { throw $Description }
}
function Set-MatrixIdentity([long]$Id, [string]$Name, [string]$Color, [string]$Key) {
    $null=Send-MatrixStep @{action='edit';tab=$Id}
    $null=Send-MatrixStep @{action='name';name=$Name}
    $null=Send-MatrixStep @{action='accent';accent=$Color}
    $null=Send-MatrixStep @{action='capture_shortcut'}
    $null=Send-MatrixKey $Key
    $null=Send-MatrixStep @{action='apply_edit'}
}
function Read-RowManifest {
    Get-Content -LiteralPath (Join-Path $EvidenceRoot "row-$Row.json") -Raw | ConvertFrom-Json
}
function Test-LinuxPid([long]$LinuxPid) {
    & "$env:WINDIR\System32\wsl.exe" --distribution Ubuntu-24.04 --exec /bin/kill -0 "$LinuxPid" 2>$null
    $LASTEXITCODE -eq 0
}

$rows = @(
    @{distro='Ubuntu-24.04';shell='bash';tag='24-bash'},
    @{distro='Ubuntu-24.04';shell='zsh';tag='24-zsh'},
    @{distro='Ubuntu-26.04';shell='bash';tag='26-bash'},
    @{distro='Ubuntu-26.04';shell='zsh';tag='26-zsh'},
    @{distro='';shell='powershell';tag='ps51'}
)
$target=$rows[$Row]

if($Phase -eq 'row-start') {
    $state=if($target.distro){Send-MatrixStep @{action='open_wsl';distro=$target.distro;shell=$target.shell}}else{Send-MatrixStep @{action='open_powershell'}}
    $id=[long]$state.active
    @{id=$id;distro=$target.distro;shell=$target.shell;tag=$target.tag;file="/home/patrick/hoo12-$([IO.Path]::GetFileName($EvidenceRoot))-$($target.tag).txt"} | ConvertTo-Json | Set-Content (Join-Path $EvidenceRoot "row-$Row.json") -Encoding utf8
    if($target.distro){
        Send-MatrixText ('printf ''\033[2J\033[H\033]2;HOO12 {0}\007''; printf ''HOO_ROW:%s\n'' ''{0}''; printf ''UTF8:%s\n'' ''Grüße 界''; printf ''\033[31mRED\033[32m GREEN\033[34m BLUE\033[0m\n''; printf ''SHELL_PID:%s\n'' $$; printf ''SHELL_VERSION:%s\n'' "${{BASH_VERSION:-$ZSH_VERSION}}"' -f $target.tag) -Enter
    }else{
        Send-MatrixText "Clear-Host; Write-Output ('HOO_ROW:' + 'ps51'); Write-Output ('UTF8:' + 'Grüße 界'); Write-Host 'RED' -ForegroundColor Red -NoNewline; Write-Host ' GREEN' -ForegroundColor Green -NoNewline; Write-Host ' BLUE' -ForegroundColor Blue; Write-Output ('SHELL_VERSION:' + `$PSVersionTable.PSVersion); Write-Output ('SHELL_PID:' + `$PID)" -Enter
    }
    $marker="HOO_ROW:$($target.tag)"
    $versionMarker=if($target.distro){'SHELL_VERSION:'}else{'SHELL_VERSION:5.1.'}
    # PTY output may span several UI ticks; the earlier UTF8 marker is not a
    # completion signal for the later version line, especially in PowerShell.
    $state=Wait-Matrix {param($s) (Test-MatrixText $s $id $marker) -and (Test-MatrixText $s $id 'UTF8:Grüße 界') -and (Test-MatrixText $s $id $versionMarker)} "$marker UTF8 and shell version"
    Save-Checkpoint "row-$Row-colors" $state
    return
}
if($Phase -in @('row-vim','row-finish')){
    $manifest=Read-RowManifest
    $id=[long]$manifest.id
    $null=Send-MatrixStep @{action='activate';tab=$id}
    if($Phase -eq 'row-vim'){
        Send-MatrixText "vim -Nu NONE -n $($manifest.file)" -Enter
        $null=Wait-Matrix {param($s) (Get-MatrixTab $s $id).alternate} 'Vim alternate screen'
        Send-MatrixText 'iGrüße 界'
        $null=Send-MatrixStep @{action='resize';width=1120.0;height=780.0}
        $state=Wait-Matrix {param($s) $t=Get-MatrixTab $s $id; $t.alternate -and $t.columns -gt 100 -and (Test-MatrixText $s $id 'Grüße 界')} 'Vim Unicode and native resize'
        Save-Checkpoint "row-$Row-vim" $state
        return
    }
    if($manifest.distro){
        $null=Send-MatrixKey Escape
        Send-MatrixText ':wq' -Enter
        $null=Wait-Matrix {param($s) -not (Get-MatrixTab $s $id).alternate} 'Return from alternate screen'
        $bytes=[IO.File]::ReadAllBytes("\\wsl.localhost\$($manifest.distro)$($manifest.file.Replace('/','\'))")
        Assert-Matrix ([Convert]::ToHexString($bytes) -eq '4772C3BCC39F6520E7958C0A') 'Vim UTF8 file bytes differ'
        Send-MatrixText "printf 'EDIT:%s\n' 'abXc'"
    }else{
        Send-MatrixText "Write-Output 'EDIT:abXc'"
    }
    $null=Send-MatrixKey Left; $null=Send-MatrixKey Left; $null=Send-MatrixKey Backspace; $null=Send-MatrixKey End; $null=Send-MatrixKey Enter
    $null=Wait-Matrix {param($s) Test-MatrixText $s $id 'EDIT:abc'} 'Interactive line editing'
    if($manifest.distro){Send-MatrixText "printf 'CTRL_START:%s\n' yes; sleep 30" -Enter}else{Send-MatrixText "Write-Output ('CTRL_START:' + 'yes'); Start-Sleep -Seconds 30" -Enter}
    $null=Wait-Matrix {param($s) Test-MatrixText $s $id 'CTRL_START:yes'} 'Interruptible command started'
    $null=Send-MatrixKey c -Ctrl
    $null=Send-MatrixStep @{action='resize';width=1000.0;height=700.0}
    if($manifest.distro){Send-MatrixText "stty size; printf 'CTRL_END:%s\n' yes" -Enter}else{Send-MatrixText "Write-Output ('CTRL_END:' + 'yes'); Write-Output ('CONSOLE_WIDTH:' + [Console]::WindowWidth)" -Enter}
    $state=Wait-Matrix {param($s) Test-MatrixText $s $id 'CTRL_END:yes'} 'Ctrl+C, resize and return to prompt'
    Save-Checkpoint "row-$Row-controls" $state
    Send-MatrixText 'exit 7' -Enter
    $state=Wait-Matrix {param($s) (Get-MatrixTab $s $id).state -eq 'Exited(7)'} 'Visible exit code 7'
    Save-Checkpoint "row-$Row-exit" $state
    return
}

$mixedPath=Join-Path $EvidenceRoot 'mixed-tabs.json'
if($Phase -eq 'mixed-start'){
    # Independent WSL work exists before the app opens its mixed sessions.
    $linuxRoot=(& "$env:WINDIR\System32\wsl.exe" -d Ubuntu-24.04 --exec /usr/bin/wslpath -a -u $EvidenceRoot).Trim()
    [IO.File]::WriteAllText((Join-Path $EvidenceRoot 'foreign.sh'), "printf '%s\n' `$$ > `"`$1`"`nexec sleep 1200`n", [Text.UTF8Encoding]::new($false))
    $start=[Diagnostics.ProcessStartInfo]::new("$env:WINDIR\System32\wsl.exe")
    $start.UseShellExecute=$false; $start.CreateNoWindow=$true; $start.WindowStyle='Hidden'
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    foreach($argument in @('--distribution','Ubuntu-24.04','--exec','/bin/sh',"$linuxRoot/foreign.sh","$linuxRoot/foreign.pid")){$start.ArgumentList.Add($argument)}
    $foreign=[Diagnostics.Process]::Start($start)
    $foreign.Id | Set-Content (Join-Path $EvidenceRoot 'foreign-helper.pid')
    $deadline=[DateTime]::UtcNow.AddSeconds(15)
    while(-not (Test-Path (Join-Path $EvidenceRoot 'foreign.pid')) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 100}
    $foreignPid=[long](Get-Content (Join-Path $EvidenceRoot 'foreign.pid'))
    Assert-Matrix (Test-LinuxPid $foreignPid) 'Independent WSL control session did not start'
    $a=Send-MatrixStep @{action='open_wsl';distro='Ubuntu-24.04';shell='bash'}; $first=[long]$a.active
    $a=Send-MatrixStep @{action='open_wsl';distro='Ubuntu-24.04';shell='zsh'}; $second=[long]$a.active
    $a=Send-MatrixStep @{action='open_powershell'}; $third=[long]$a.active
    @{first=$first;second=$second;third=$third} | ConvertTo-Json | Set-Content $mixedPath -Encoding utf8
    Set-MatrixIdentity $first 'WSL Bash · Grün' 'green' 'F8'
    Set-MatrixIdentity $second 'WSL zsh · Türkis' 'cyan' 'F9'
    Set-MatrixIdentity $third 'PowerShell · Lila' 'purple' 'F10'
    foreach($old in 1..5){$null=Send-MatrixStep @{action='close';tab=$old}}
    foreach($id in @($first,$second)){
        $null=Send-MatrixStep @{action='activate';tab=$id}
        Send-MatrixText ('printf ''MIXED:%s\n'' ''{0}''; printf ''SHELL_PID:%s\n'' $$; printf ''\033]2;dynamic-{0}\007''' -f $id) -Enter
        $marker="MIXED:$id"; $null=Wait-Matrix {param($s) Test-MatrixText $s $id $marker} 'Mixed WSL output'
    }
    $null=Send-MatrixStep @{action='activate';tab=$third}
    Send-MatrixText "Write-Output ('MIXED:' + '$third'); Write-Output ('PS_VERSION:' + `$PSVersionTable.PSVersion)" -Enter
    $marker="MIXED:$third"; $state=Wait-Matrix {param($s) Test-MatrixText $s $third $marker} 'Mixed native PowerShell'
    Save-Checkpoint 'mixed-start' $state
    return
}
$mixed=Get-Content $mixedPath -Raw | ConvertFrom-Json
$first=[long]$mixed.first; $second=[long]$mixed.second; $third=[long]$mixed.third
if($Phase -eq 'mixed-shortcuts'){
    $null=Send-MatrixStep @{action='edit';tab=$second}; $null=Send-MatrixStep @{action='capture_shortcut'}
    $s=Send-MatrixKey t -Ctrl -Shift; Assert-Matrix ([bool]$s.editor_error) 'Built-in shortcut conflict missing'
    $s=Send-MatrixKey F8; Assert-Matrix ([bool]$s.editor_error) 'Duplicate shortcut conflict missing'
    $null=Send-MatrixStep @{action='cancel_edit'}
    $null=Send-MatrixStep @{action='move';source=$first;target=$third;after=$true}
    $s=Send-MatrixKey F8; Assert-Matrix ($s.active -eq $first) 'Shortcut lost stable identity after move'
    Assert-Matrix ((Get-MatrixTab $s $first).identity.accent -eq 'green') 'Accent moved to a different tab'
    Send-MatrixText "printf 'RAW_READY:%s\n' yes; stty -echo -icanon min 1 time 0; dd bs=1 count=1 2>/dev/null | od -An -tx1; stty sane; printf 'RAW_DONE:%s\n' yes" -Enter
    $null=Wait-Matrix {param($s) Test-MatrixText $s $first 'RAW_READY:yes'} 'Raw byte probe ready'
    $s=Send-MatrixKey F10; Assert-Matrix ($s.active -eq $third) 'Direct mixed tab shortcut failed'
    $s=Send-MatrixKey Tab -Ctrl; Assert-Matrix ($s.active -eq $first) 'Built-in tab cycle failed'
    Send-MatrixText 'q'
    $s=Wait-Matrix {param($s) Test-MatrixText $s $first 'RAW_DONE:yes'} 'Raw byte probe completed'
    Assert-Matrix ([bool]((Get-MatrixTab $s $first).screen | Where-Object {$_ -match '^\s*71\s*$'})) 'Recognized app shortcut leaked to PTY'
    Send-MatrixText "printf 'CLIP_%s\n' 'Grüße'" -Enter
    $s=Wait-Matrix {param($s) Test-MatrixText $s $first 'CLIP_Grüße'} 'Synthetic clipboard source'
    $lines=(Get-MatrixTab $s $first).screen; $clipRow=-1
    for($i=0;$i -lt $lines.Count;$i++){if($lines[$i].TrimEnd() -eq 'CLIP_Grüße'){$clipRow=$i;break}}
    Assert-Matrix ($clipRow -ge 0) 'Clipboard selection row missing'
    $null=Send-MatrixStep @{action='select';tab=$first;row=$clipRow;column=0;start=$true;right=$false}
    $null=Send-MatrixStep @{action='select';tab=$first;row=$clipRow;column=9;start=$false;right=$true}
    $null=Send-MatrixKey c -Ctrl -Shift
    $null=Send-MatrixStep @{action='check_clipboard';expected='CLIP_Grüße'}
    $null=Wait-Matrix {param($s) $s.clipboard_matches -eq $true} 'Native Clipboard copy from WSL tab'
    $null=Send-MatrixKey F10
    Send-MatrixText "Write-Output ('PASTE:' + '"
    $null=Send-MatrixKey v -Ctrl -Shift
    Start-Sleep -Milliseconds 250
    Send-MatrixText "')" -Enter
    $s=Wait-Matrix {param($s) Test-MatrixText $s $third 'PASTE:CLIP_Grüße'} 'Native Clipboard paste into PowerShell'
    Save-Checkpoint 'mixed-shortcuts-clipboard' $s
    return
}
if($Phase -eq 'mixed-load'){
    $process=Get-Process -Id ([int](Get-Content (Join-Path $EvidenceRoot 'pid.txt')))
    $cpuStart=$process.TotalProcessorTime.TotalSeconds; $watch=[Diagnostics.Stopwatch]::StartNew()
    $null=Send-MatrixKey F8
    Send-MatrixText 'for i in {1..6000}; do printf ''LOAD:%05d\n'' "$i"; if (( i % 100 == 0 )); then sleep 0.05; fi; done; printf ''LOAD_DONE:%s\n'' yes' -Enter
    $null=Send-MatrixKey F10
    Send-MatrixText "Write-Output ('UI_DURING_LOAD:' + 'ready')" -Enter
    $null=Wait-Matrix {param($s) Test-MatrixText $s $third 'UI_DURING_LOAD:ready'} 'Interactive native tab during inactive WSL output'
    foreach($i in 0..11){$null=Send-MatrixStep @{action='resize';width=[float](800+20*$i);height=[float](600+10*$i)}}
    $null=Send-MatrixKey F8
    $s=Wait-Matrix {param($s) Test-MatrixText $s $first 'LOAD_DONE:yes'} 'Inactive output drained'
    Assert-Matrix ((Get-MatrixTab $s $first).history -eq 2000) 'Scrollback is not bounded at 2000 lines'
    $null=Send-MatrixKey Home -Ctrl -Shift
    $s=Send-MatrixStep @{action='observe'}
    Assert-Matrix ((Get-MatrixTab $s $first).offset -eq 2000) 'History top shortcut failed'
    Assert-Matrix (-not (Test-MatrixText $s $first 'LOAD:00001')) 'Discarded old line remains visible'
    Save-Checkpoint 'mixed-history-top' $s
    $null=Send-MatrixKey PageDown -Shift; $null=Send-MatrixKey PageUp -Shift
    $null=Send-MatrixKey End -Ctrl -Shift
    $s=Send-MatrixStep @{action='observe'}; Assert-Matrix ((Get-MatrixTab $s $first).offset -eq 0) 'Live shortcut failed'
    $process.Refresh(); $watch.Stop()
    @{wallSeconds=$watch.Elapsed.TotalSeconds;appCpuSeconds=($process.TotalProcessorTime.TotalSeconds-$cpuStart);workingSetBytes=$process.WorkingSet64;privateBytes=$process.PrivateMemorySize64;logicalProcessors=[Environment]::ProcessorCount;scope='Native Windows fixture process only; WSL CPU/memory excluded'} | ConvertTo-Json | Set-Content (Join-Path $EvidenceRoot 'load-metrics.json') -Encoding utf8
    Save-Checkpoint 'mixed-load-complete' $s
    return
}
if($Phase -eq 'mixed-close'){
    $null=Send-MatrixKey F8
    Send-MatrixText 'sleep 120 & printf ''JOB_PID:%s\n'' $!; printf ''CLOSE_SHELL_PID:%s\n'' $$; printf ''CLOSE_READY:%s\n'' yes' -Enter
    $s=Wait-Matrix {param($s) Test-MatrixText $s $first 'CLOSE_READY:yes'} 'Owned background process started'
    Save-Checkpoint 'mixed-before-close' $s
    $screen=(Get-MatrixTab $s $first).screen -join "`n"
    $ownedShell=[long]([regex]::Match($screen,'(?m)^CLOSE_SHELL_PID:(\d+)').Groups[1].Value)
    $ownedJob=[long]([regex]::Match($screen,'(?m)^JOB_PID:(\d+)').Groups[1].Value)
    $foreignPid=[long](Get-Content (Join-Path $EvidenceRoot 'foreign.pid'))
    Assert-Matrix ((Test-LinuxPid $ownedShell) -and (Test-LinuxPid $ownedJob) -and (Test-LinuxPid $foreignPid)) 'Close control PIDs not alive before close'
    $null=Send-MatrixStep @{action='close';tab=$first}
    $s=Send-MatrixStep @{action='observe'}; Assert-Matrix $s.confirming 'Named tab confirmation missing'
    Save-Checkpoint 'mixed-close-confirmation' $s
    $null=Send-MatrixStep @{action='cancel'}
    $s=Send-MatrixStep @{action='observe'}; Assert-Matrix ((Get-MatrixTab $s $first).state -eq 'Running') 'Canceled close lost the session'
    Assert-Matrix (Test-LinuxPid $ownedShell) 'Canceled close terminated its Linux shell'
    $null=Send-MatrixStep @{action='close';tab=$first}; $null=Send-MatrixStep @{action='confirm'}
    $s=Wait-Matrix {param($s) $s.tabs.Count -eq 2 -and $s.pending_shutdowns -eq 0} 'Targeted close completed'
    foreach($id in @($second,$third)){Assert-Matrix ((Get-MatrixTab $s $id).state -eq 'Running') 'Other mixed tab was terminated'}
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    while((Test-LinuxPid $ownedShell) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 100}
    Assert-Matrix (-not (Test-LinuxPid $ownedShell)) 'Closed Linux shell survived'
    Assert-Matrix (-not (Test-LinuxPid $ownedJob)) 'Owned Linux background job survived'
    Assert-Matrix (Test-LinuxPid $foreignPid) 'Independent WSL work was terminated'
    $null=Send-MatrixKey F9
    Send-MatrixText "printf 'SURVIVOR:%s\n' yes" -Enter
    $s=Wait-Matrix {param($s) Test-MatrixText $s $second 'SURVIVOR:yes'} 'Other WSL tab remains interactive'
    Save-Checkpoint 'mixed-close-survivors' $s
    $null=Send-MatrixStep @{action='close';tab=$null}
    $s=Send-MatrixStep @{action='observe'}; Assert-Matrix $s.confirming 'Window confirmation missing'
    Save-Checkpoint 'mixed-window-confirmation' $s
    $null=Send-MatrixStep @{action='cancel'}
    return
}
if($Phase -eq 'errors'){
    $s=Send-MatrixStep @{action='open_missing';wsl=$false}; $failed=[long]$s.active
    Assert-Matrix ((Get-MatrixTab $s $failed).state.StartsWith('Failed(')) 'Missing executable failure not visible'
    $s=Send-MatrixStep @{action='close';tab=$failed}; Assert-Matrix (-not $s.confirming) 'Failed process required confirmation'
    $s=Send-MatrixStep @{action='open_missing';wsl=$true}; $failed=[long]$s.active
    $s=Wait-Matrix {param($s) (Get-MatrixTab $s $failed).state -ne 'Running'} 'Missing distribution exits visibly'
    Save-Checkpoint 'missing-target' $s
    $s=Send-MatrixStep @{action='close';tab=$failed}; Assert-Matrix (-not $s.confirming) 'Exited missing distribution required confirmation'
    Save-Checkpoint 'errors-complete' $s
}
