# Quick Start: Adding winget-tui to Winget

> **TL;DR:** Use `wingetcreate` to generate manifests, submit a PR to microsoft/winget-pkgs.

## Fast Track (Recommended)

### 1. Install the tool
```powershell
winget install Microsoft.WingetCreate
```

### 2. Generate manifests
```powershell
wingetcreate new https://github.com/shanselman/winget-tui/releases/download/v0.1.3/winget-tui-x64.exe
```

Follow the interactive prompts:
- Package ID: `Shanselman.WingetTUI`
- Publisher: `Scott Hanselman`
- App Name: `winget-tui`
- Version: `0.1.3`
- License: `MIT`
- Homepage: `https://github.com/shanselman/winget-tui`

The tool will generate three YAML files.

### 3. Add ARM64 support
Edit the generated `Shanselman.WingetTUI.installer.yaml` to add:
```yaml
  - Architecture: arm64
    InstallerUrl: https://github.com/shanselman/winget-tui/releases/download/v0.1.3/winget-tui-arm64.exe
    InstallerSha256: <hash from Get-FileHash>
```

### 4. Fork and submit
```bash
# Fork microsoft/winget-pkgs on GitHub first
git clone https://github.com/YOUR-USERNAME/winget-pkgs.git
cd winget-pkgs
git checkout -b shanselman-wingettui-0.1.3

# Create directory and copy manifests
mkdir -p manifests/s/Shanselman/WingetTUI/0.1.3
cp Shanselman.WingetTUI.* manifests/s/Shanselman/WingetTUI/0.1.3/

# Commit and push
git add manifests/s/Shanselman/WingetTUI/
git commit -m "New package: Shanselman.WingetTUI version 0.1.3"
git push origin shanselman-wingettui-0.1.3
```

### 5. Create PR
Open a pull request from your fork to `microsoft/winget-pkgs`.

## For Updates (v0.1.4+)

```powershell
wingetcreate update Shanselman.WingetTUI --version 0.1.4 --urls https://github.com/shanselman/winget-tui/releases/download/v0.1.4/winget-tui-x64.exe https://github.com/shanselman/winget-tui/releases/download/v0.1.4/winget-tui-arm64.exe
```

## Need More Details?

See [WINGET_SUBMISSION.md](WINGET_SUBMISSION.md) for comprehensive instructions.

## After Acceptance

Update README.md to change "Coming Soon" to:

```markdown
### Via Winget

```powershell
winget install Shanselman.WingetTUI
```
```
