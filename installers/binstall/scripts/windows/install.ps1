# Licensed under the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be downloaded from the internet to
# install rover. It downloads the rover tarball from GitHub releases,
# extracts it and runs `rover install $Args`. This means that you can pass
# arguments to this shell script and they will be passed along to the installer.

# Example to bypass binary overwrite [y/N] prompt
# iwr https://rover.apollo.dev/win/latest | iex --force

# version found in Rover's Cargo.toml
# Note: this line is built automatically
# in build.rs. Don't touch it!
$package_version = 'v0.34.1'

function Install-Binary($rover_install_args) {
  $old_erroractionpreference = $ErrorActionPreference
  $ErrorActionPreference = 'stop'

  Initialize-Environment

  # If the VERSION env var is set, we use it instead
  # of the version defined in Rover's cargo.toml
  $download_version = if (Test-Path env:VERSION) {
    $Env:VERSION
  } else {
    $package_version
  }

  $exe = Download($download_version)
  Invoke-Installer "$exe" "$rover_install_args"

  $ErrorActionPreference = $old_erroractionpreference
}

function Download($version) {
  $binary_download_prefix = $env:APOLLO_ROVER_BINARY_DOWNLOAD_PREFIX
  if (-not $binary_download_prefix) {
    $binary_download_prefix = "https://github.com/apollographql/rover/releases/download"
  }
  $url = "$binary_download_prefix/$version/rover-$version-x86_64-pc-windows-msvc.tar.gz"

  # Remove credentials from the URL for logging
  $safe_url = $url -replace "https://[^@]+@", "https://"

  "Downloading Rover from $safe_url" | Out-Host
  $tmp = New-Temp-Dir
  $dir_path = "$tmp\rover.tar.gz"
  $wc = New-Object Net.Webclient
  $wc.downloadFile($url, $dir_path)
  tar -xkf $dir_path -C "$tmp"
  return "$tmp\dist\rover.exe"
}

function Invoke-Installer($tmp, $rover_install_args) {
  if (![string]::IsNullOrWhiteSpace($rover_install_args)) {
    & "$exe" "install" "$rover_install_args"
  } else {
    & "$exe" "install"
  }
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

  If (-Not (Get-Command 'tar')) {
    Write-Error "The tar command is not installed on this machine. Please install tar before installing Rover"
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

Install-Binary "$Args"
