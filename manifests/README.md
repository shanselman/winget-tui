# Winget Manifest Examples

This directory contains example winget manifest files for submitting winget-tui to the Windows Package Manager Community Repository.

## Files

- **Shanselman.WingetTUI.yaml** - Version manifest (required)
- **Shanselman.WingetTUI.installer.yaml** - Installer details with SHA256 hashes (required)
- **Shanselman.WingetTUI.locale.en-US.yaml** - English locale information (required)

## Usage

These are **example/template** files. Before submitting to winget-pkgs:

1. **Update the version number** in all three files to match your release
2. **Calculate SHA256 hashes** for both x64 and ARM64 executables:
   ```powershell
   Get-FileHash winget-tui-x64.exe -Algorithm SHA256
   Get-FileHash winget-tui-arm64.exe -Algorithm SHA256
   ```
3. **Replace placeholders** in the installer.yaml file with actual hashes
4. **Update URLs** to point to the correct release version
5. **Update ReleaseDate** in installer.yaml
6. **Update ReleaseNotesUrl** in locale file

## Validation

Before submitting, validate locally:

```powershell
winget validate --manifest path\to\manifests\example\
```

## Submission

See [WINGET_SUBMISSION.md](../WINGET_SUBMISSION.md) for complete instructions on submitting these manifests to the winget-pkgs repository.

## Using wingetcreate

Instead of manually editing these files, you can use the `wingetcreate` tool:

```powershell
# For new package
wingetcreate new https://github.com/shanselman/winget-tui/releases/download/v0.1.3/winget-tui-x64.exe

# For updating existing package
wingetcreate update Shanselman.WingetTUI --version 0.1.4 --urls https://github.com/shanselman/winget-tui/releases/download/v0.1.4/winget-tui-x64.exe
```

The tool will automatically calculate hashes and generate properly formatted YAML files.
