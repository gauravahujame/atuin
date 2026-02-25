# Maintenance and Releases

This document outlines the process for releasing new versions of Atuin and updating the Homebrew tap.

## Releasing a New Version

We use a automated release pipeline that builds artifacts, syncs to Gitea, and updates the Homebrew tap.

### Prerequisites

1.  Ensure you have the `release` branch checked out.
2.  Ensure your local `release` branch is up to date with `upstream/main`.

### Release Steps

To release a new version, use the `release.sh` script in the root of the repository.

1.  **Run the release script**:

    You can either let the script detect the version from `Cargo.toml` or provide it manually.

    ```bash
    # Auto-detect version and add -patch suffix
    ./release.sh auto

    # OR provide a specific version (must end in -patch)
    ./release.sh v18.13.0-beta.2
    ```

2.  **Verify the actions**:
    The script will:
    - Update the version in `Cargo.toml`.
    - Commit the change.
    - Create a git tag (e.g., `v18.13.0-beta.2-patch`).
    - Push the branch and tag to your GitHub fork (`origin`).

3.  **Monitor the Release**:
    - The `release.sh` script will calculate the release SHA256, update your local `homebrew-tap` formula, commit it, and push it to all configured remotes for the tap.
    - Go to the **Actions** tab of your GitHub repository.
    - The `release.yml` workflow will start building artifacts and publish a GitHub release.
    - The `sync-release.yml` workflow will then trigger to sync the release to your Gitea instance.

## Updating Atuin via Homebrew

Once the automated release process is complete (usually after 5-10 minutes), users can update their local installation.

### Commands

```bash
# Update Homebrew formulae and Atuin
brew update
brew upgrade gaurav/tap/atuin

# If you are using the Atuin daemon, restart the service
brew services restart gaurav/tap/atuin
```

### Verifying the Update

You can verify the installed version by running:

```bash
atuin --version
```

And check the daemon health:

```bash
atuin daemon check
```
