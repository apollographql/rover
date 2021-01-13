function Install-Binary() {
  $old_erroractionpreference = $ErrorActionPreference
  $ErrorActionPreference = 'stop'

  $version = "0.0.1-rc.3"

  Initialize-Environment

  $exe = Download($version)

  Invoke-Installer($exe)

  $ErrorActionPreference = $old_erroractionpreference
}

function Download($version) {
  $url = "https://github.com/apollographql/rover/releases/download/v$version/rover-v$version-x86_64-pc-windows-msvc.tar.gz"
  "Downloading Rover from $url" | Out-Host
  $tmp = New-Temp-Dir
  $dir_path = "$tmp\rover.tar.gz"
  $wc = New-Object Net.Webclient
  $wc.downloadFile($url, $dir_path)
  tar -xkf $dir_path -C "$tmp"
  return "$tmp"
}

function Invoke-Installer($tmp) {
  $exe = "$tmp\dist\rover.exe"
  & "$exe" "install"
  Remove-Item "$tmp" -Recurse -Force
}

function Initialize-Environment() {
  If (($PSVersionTable.PSVersion.Major) -lt 5) {
    Write-Error "PowerShell 5 or later is required to install Rover."
    Write-Error "Upgrade PowerShell: https://docs.microsoft.com/en-us/powershell/scripting/setup/installing-windows-powershell"
    break
  }

  # show notification to change execution policy:
  $allowedExecutionPolicy = @('Unrestricted', 'RemoteSigned', 'ByPass')
  If ((Get-ExecutionPolicy).ToString() -notin $allowedExecutionPolicy) {
    Write-Error "PowerShell requires an execution policy in [$($allowedExecutionPolicy -join ", ")] to run Rover."
    Write-Error "For example, to set the execution policy to 'RemoteSigned' please run :"
    Write-Error "'Set-ExecutionPolicy RemoteSigned -scope CurrentUser'"
    break
  }

  # GitHub requires TLS 1.2
  If ([System.Enum]::GetNames([System.Net.SecurityProtocolType]) -notcontains 'Tls12') {
    Write-Error "Installing Rover requires at least .NET Framework 4.5"
    Write-Error "Please download and install it first:"
    Write-Error "https://www.microsoft.com/net/download"
    break
  }

  If (-Not (Get-Command 'curl')) {
    Write-Error "The curl command is not installed on this machine. Please install curl before installing Rover"
    # don't abort if invoked with iex that would close the PS session
    If ($myinvocation.mycommand.commandtype -eq 'Script') { return } else { exit 1 }
  }

  If (-Not (Get-Command 'tar')) {
    Write-Error "The tar command is not installed on this machine. Please install curl before installing Rover"
    # don't abort if invoked with iex that would close the PS session
    If ($myinvocation.mycommand.commandtype -eq 'Script') { return } else { exit 1 }
  }
}

function New-Temp-Dir() {
  [CmdletBinding(SupportsShouldProcess)]
  param()
  $parent = [System.IO.Path]::GetTempPath()
  [string] $name = [System.Guid]::NewGuid()
  New-Item -ItemType Directory -Path (Join-Path $parent $name)
}

Install-Binary
