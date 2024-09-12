$ErrorActionPreference = "Stop"

Function New-TemporaryFolder {
    # Make a new folder based upon a TempFileName
    $TEMP_PATH=[System.IO.Path]::GetTempPath()
    $T= Join-Path $TEMP_PATH tmp$([convert]::tostring((get-random 65535),16).padleft(4,'0'))
    New-Item -ItemType Directory -Path $T
}

# Find the installers directory
$installer_dir = [System.IO.Path]::Combine($PSScriptRoot, "..", "..", "..", "installers", "npm")
Write-Output "Found installers directory at $installer_dir"

# Create a temporary folder for the test
$test_dir = New-TemporaryFolder
Write-Output "Created test directory at $test_dir"
Set-Location $test_dir
# Install pnpm
npm install -g pnpm@v9.3.0

# The choice of version here is arbitrary (we just need something we know exists) so that we can test if the
# installer works, given an existing version. This way we're not at the mercy of whether the binary that corresponds
# to the latest commit exists.
npm --prefix "$installer_dir" version --allow-same-version 0.23.0
Write-Output "Temporarily patched package.json to fixed stable binary"

# Install all the dependencies, including `rover`
pnpm init
pnpm add "file:$installer_dir"
Write-Output "Installed rover as local npm package"

# Move to the installed location
$node_modules_path=[System.IO.Path]::Combine($test_dir, "node_modules", ".bin")
Set-Location $node_modules_path

# Check the version
Write-Output "Checking version"
$dir_sep=[IO.Path]::DirectorySeparatorChar
$rover_command=".${dir_sep}rover --version"
Invoke-Expression $rover_command
Write-Output "Checked version, all ok!"
