# Winget release checklist

This repo publishes signed release binaries on every `v*` tag. Winget publishing
adds two extra pieces:

1. versioned portable ZIP assets for Winget manifests
2. a manifest submission PR to `microsoft/winget-pkgs`

## First-time package setup

Package identity:

- PackageIdentifier: `Hanselman.WingetTUI`
- PackageName: `winget-tui`
- Moniker/command: `winget-tui`
- Publisher: `Scott Hanselman`

The first submission PR is:

- https://github.com/microsoft/winget-pkgs/pull/375827

## Required secret

The automated workflow uses a repository secret named:

```text
WINGET_CREATE_GITHUB_TOKEN
```

Use a token that is accepted by the `microsoft/winget-pkgs` organization policy.
Classic PATs with a lifetime longer than 90 days are rejected by Microsoft Open
Source policy.

## Release asset requirements

The Build workflow must upload these release assets:

```text
winget-tui-x64.exe
winget-tui-arm64.exe
winget-tui-<version>-x64.zip
winget-tui-<version>-arm64.zip
```

Each ZIP contains a nested folder with a stable executable name and the project
MIT license:

```text
winget-tui-<version>-x64\winget-tui.exe
winget-tui-<version>-x64\LICENSE
winget-tui-<version>-arm64\winget-tui.exe
winget-tui-<version>-arm64\LICENSE
```

## Normal release flow

1. Cut and publish the GitHub release as usual by pushing a `v*` tag.
2. Confirm the release has both `.exe` and `.zip` assets.
3. Run the **Winget Submit** workflow with the release tag, for example:

```powershell
gh workflow run winget-submit.yml --repo shanselman/winget-tui --ref main -f tag=v0.11.0
```

4. Watch the workflow:

```powershell
gh run list --repo shanselman/winget-tui --workflow "Winget Submit" --limit 5
```

5. Track the resulting `microsoft/winget-pkgs` PR until it is merged.

## Manual fallback

If `wingetcreate submit` is blocked by token policy, generate the manifest from
the same release asset URLs and open a PR from a fork of `microsoft/winget-pkgs`.
Validate before opening the PR:

```powershell
winget validate --manifest <manifest-folder>
```

The manifest path for this package is:

```text
manifests\h\Hanselman\WingetTUI\<version>
```

Use manifest schema `1.10.0` for now because it validates cleanly on current
GitHub-hosted Windows runners while supporting the portable ZIP fields used by
this package.
