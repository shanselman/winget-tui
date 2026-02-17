# Adding winget-tui to Windows Package Manager

This guide explains how to submit winget-tui to the [Windows Package Manager Community Repository](https://github.com/microsoft/winget-pkgs) so users can install it via `winget install winget-tui`.

## Prerequisites

Before submitting to winget, ensure:

1. ✅ The application has a stable release with signed executables
2. ✅ Release artifacts are published on GitHub Releases
3. ✅ Each release has a consistent URL pattern
4. ✅ SHA256 hashes can be calculated for installers

## Overview of the Process

Adding winget-tui to winget involves:

1. Creating YAML manifest files that describe the package
2. Forking the winget-pkgs repository
3. Adding the manifest files to the correct directory structure
4. Submitting a pull request
5. Addressing any automated validation feedback

## Step-by-Step Instructions

### 1. Install winget-create Tool

Microsoft provides a tool to help create manifests:

```powershell
winget install Microsoft.WingetCreate
```

Or download from: https://github.com/microsoft/winget-create

### 2. Generate Manifest Files

Use the `wingetcreate` tool to generate manifest files interactively:

```powershell
wingetcreate new https://github.com/shanselman/winget-tui/releases/download/v0.1.3/winget-tui-x64.exe
```

This will prompt you for:
- Package Identifier (e.g., `Shanselman.WingetTUI`)
- Publisher name
- Application name
- Version
- License
- Description
- Homepage URL
- And more...

The tool will automatically:
- Download the installer
- Calculate SHA256 hash
- Create the required YAML files

### 3. Manual Manifest Creation

Alternatively, create three YAML files manually (see `manifests/` directory for examples):

**Required files:**
- `Shanselman.WingetTUI.installer.yaml` - Installer details
- `Shanselman.WingetTUI.locale.en-US.yaml` - English locale information
- `Shanselman.WingetTUI.yaml` - Version manifest

### 4. Fork and Clone winget-pkgs Repository

```bash
# Fork https://github.com/microsoft/winget-pkgs on GitHub first

git clone https://github.com/YOUR-USERNAME/winget-pkgs.git
cd winget-pkgs
```

### 5. Create Directory Structure

```bash
# For first-time submission (new package)
mkdir -p manifests/s/Shanselman/WingetTUI/0.1.3

# Copy your manifest files
cp Shanselman.WingetTUI.yaml manifests/s/Shanselman/WingetTUI/0.1.3/
cp Shanselman.WingetTUI.installer.yaml manifests/s/Shanselman/WingetTUI/0.1.3/
cp Shanselman.WingetTUI.locale.en-US.yaml manifests/s/Shanselman/WingetTUI/0.1.3/
```

**Directory structure explained:**
- `manifests/` - Root directory for all manifests
- `s/` - First letter of publisher name
- `Shanselman/` - Publisher name
- `WingetTUI/` - Package name
- `0.1.3/` - Version number

### 6. Validate Manifest Locally

Before submitting, test your manifest:

```powershell
# Install the winget validation tool
winget install Microsoft.WinGet.Client

# Validate manifest
winget validate --manifest path\to\manifests\s\Shanselman\WingetTUI\0.1.3\

# Test installation locally
winget install --manifest path\to\manifests\s\Shanselman\WingetTUI\0.1.3\Shanselman.WingetTUI.yaml
```

### 7. Submit Pull Request

```bash
# Create a branch
git checkout -b shanselman-wingettui-0.1.3

# Commit manifest files
git add manifests/s/Shanselman/WingetTUI/
git commit -m "New package: Shanselman.WingetTUI version 0.1.3"

# Push to your fork
git push origin shanselman-wingettui-0.1.3
```

Then open a pull request on GitHub to `microsoft/winget-pkgs`.

### 8. Automated Validation

After submitting the PR:
- Automated checks validate the manifest format
- Bots verify the installer URL is accessible
- SHA256 hash is verified
- Package metadata is checked
- SmartScreen/antivirus scans may run

Address any failures before the PR can be merged.

### 9. Updating for New Versions

For subsequent releases (e.g., v0.1.4):

```powershell
# Use wingetcreate to update existing package
wingetcreate update Shanselman.WingetTUI --version 0.1.4 --urls https://github.com/shanselman/winget-tui/releases/download/v0.1.4/winget-tui-x64.exe
```

This creates updated manifests in a new version directory.

## Important Notes

### Multi-Architecture Support

winget-tui builds for both x64 and ARM64. The installer manifest should include both:

```yaml
Installers:
  - Architecture: x64
    InstallerUrl: https://github.com/shanselman/winget-tui/releases/download/v0.1.3/winget-tui-x64.exe
    InstallerSha256: <hash>
  - Architecture: arm64
    InstallerUrl: https://github.com/shanselman/winget-tui/releases/download/v0.1.3/winget-tui-arm64.exe
    InstallerSha256: <hash>
```

### Portable vs. Installer

Since winget-tui is a standalone executable (not a traditional installer), set:

```yaml
InstallerType: portable
```

Note: `NestedInstallerType` is not used with portable installers - it's only for zip/exe containers.

Alternatively, consider creating a simple MSI/MSIX package for better winget integration and automatic PATH updates.

### Calculating SHA256 Hash

```powershell
# PowerShell
Get-FileHash winget-tui-x64.exe -Algorithm SHA256

# Or use certutil
certutil -hashfile winget-tui-x64.exe SHA256
```

### Commands Field

Specify the command users will run:

```yaml
Commands:
  - winget-tui
```

## Useful Resources

- [Official winget-pkgs repository](https://github.com/microsoft/winget-pkgs)
- [Contributing guidelines](https://github.com/microsoft/winget-pkgs/blob/master/CONTRIBUTING.md)
- [Manifest schema documentation](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest)
- [wingetcreate tool](https://github.com/microsoft/winget-create)
- [Manifest validation](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest)

## After Acceptance

Once your PR is merged:
- The package becomes available via `winget search winget-tui`
- Users can install with `winget install Shanselman.WingetTUI`
- It may take a few hours to 24 hours to appear in search results
- Update the README to include winget installation instructions

## Automation Opportunity

Consider setting up a GitHub Action to automatically:
1. Detect new releases
2. Generate updated manifests
3. Submit PRs to winget-pkgs

Example: [winget-releaser GitHub Action](https://github.com/marketplace/actions/winget-releaser)
