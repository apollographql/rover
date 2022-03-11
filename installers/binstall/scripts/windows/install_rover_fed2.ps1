# Copyright 2021 Apollo Graph, Inc.

# Elastic License 2.0

# ## Acceptance

# By using the software, you agree to all of the terms and conditions below.

# ## Copyright License

# The licensor grants you a non-exclusive, royalty-free, worldwide,
# non-sublicensable, non-transferable license to use, copy, distribute, make
# available, and prepare derivative works of the software, in each case subject to
# the limitations and conditions below.

# ## Limitations

# You may not provide the software to third parties as a hosted or managed
# service, where the service provides users with access to any substantial set of
# the features or functionality of the software.

# You may not move, change, disable, or circumvent the license key functionality
# in the software, and you may not remove or obscure any functionality in the
# software that is protected by the license key.

# You may not alter, remove, or obscure any licensing, copyright, or other notices
# of the licensor in the software. Any use of the licensor’s trademarks is subject
# to applicable law.

# ## Patents

# The licensor grants you a license, under any patent claims the licensor can
# license, or becomes able to license, to make, have made, use, sell, offer for
# sale, import and have imported the software, in each case subject to the
# limitations and conditions in this license. This license does not cover any
# patent claims that you cause to be infringed by modifications or additions to
# the software. If you or your company make any written claim that the software
# infringes or contributes to infringement of any patent, your patent license for
# the software granted under these terms ends immediately. If your company makes
# such a claim, your patent license ends immediately for work on behalf of your
# company.

# ## Notices

# You must ensure that anyone who gets a copy of any part of the software from you
# also gets a copy of these terms.

# If you modify the software, you must include in any modified copies of the
# software prominent notices stating that you have modified the software.

# ## No Other Rights

# These terms do not imply any licenses other than those expressly granted in
# these terms.

# ## Termination

# If you use the software in violation of these terms, such use is not licensed,
# and your licenses will automatically terminate. If the licensor provides you
# with a notice of your violation, and you cease all violation of this license no
# later than 30 days after you receive that notice, your licenses will be
# reinstated retroactively. However, if you violate these terms after such
# reinstatement, any additional violation of these terms will cause your licenses
# to terminate automatically and permanently.

# ## No Liability

# *As far as the law allows, the software comes as is, without any warranty or
# condition, and the licensor will not be liable to you for any damages arising
# out of these terms or the use or nature of the software, under any kind of
# legal claim.*

# ## Definitions

# The **licensor** is the entity offering these terms, and the **software** is the
# software the licensor makes available under these terms, including any portion
# of it.

# **you** refers to the individual or entity agreeing to these terms.

# **your company** is any legal entity, sole proprietorship, or other kind of
# organization that you work for, plus all organizations that have control over,
# are under the control of, or are under common control with that
# organization. **control** means ownership of substantially all the assets of an
# entity, or the power to direct its management and policies by vote, contract, or
# otherwise. Control can be direct or indirect.

# **your licenses** are all the licenses granted to you for the software under
# these terms.

# **use** means anything you do with the software requiring one of your licenses.

# **trademark** means trademarks, service marks, and similar rights.

# --------------------------------------------------------------------------------

# This is just a little script that can be downloaded from the internet to
# install rover-fed2. You must first install Rover and have it in your PATH
# in order to use this script. If you have installed via npm, you will need to uninstall
# and reinstall with the iwr | iex installer.

# This script first downloads Rover, which in turn downloads the rover-fed2 tarball
# by invoking `rover install --plugin rover-fed2 $Args`. This means that you can pass
# arguments to this shell script and they will be passed along to the installer.

# Example to bypass binary overwrite [y/N] prompt
# iwr https://rover.apollo.dev/plugins/rover-fed2/win/latest | iex --force

# Example to Accept the terms and conditions in the elv2 license
# iwr https://rover.apollo.dev/plugins/rover-fed2/win/latest | iex --elv2-license accept

# version found in Rover's Cargo.toml
# Note: this line is built automatically
# in build.rs. Don't touch it!
$package_version = 'v0.4.5'

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
  $url = "https://github.com/apollographql/rover/releases/download/$version/rover-$version-x86_64-pc-windows-msvc.tar.gz"
  "Downloading Rover from $url" | Out-Host
  $tmp = New-Temp-Dir
  $dir_path = "$tmp\rover.tar.gz"
  $wc = New-Object Net.Webclient
  $wc.downloadFile($url, $dir_path)
  tar -xkf $dir_path -C "$tmp"
  return "$tmp\dist\rover.exe"
}

function Invoke-Installer($tmp, $rover_install_args) {
  & "$exe" "install" "--plugin" "rover-fed2" "$rover_install_args"
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
