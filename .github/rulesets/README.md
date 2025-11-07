# GitHub Branch Protection Rulesets

This directory contains JSON configurations for GitHub branch protection rulesets.

## Files

- `protect-main.json` - Protection rules for `main` branch
- `protect-develop.json` - Protection rules for `develop` branch

## How to Import Rulesets

### Method 1: Using GitHub CLI (Recommended)

```bash
# Install GitHub CLI if not already installed
# macOS: brew install gh
# Other: https://cli.github.com/

# Login to GitHub
gh auth login

# Import main branch ruleset
gh api repos/leonidbkh/tierflow/rulesets \
  --method POST \
  --input .github/rulesets/protect-main.json

# Import develop branch ruleset
gh api repos/leonidbkh/tierflow/rulesets \
  --method POST \
  --input .github/rulesets/protect-develop.json
```

### Method 2: Using curl with Personal Access Token

1. Create a Personal Access Token (classic):
   - Go to: https://github.com/settings/tokens
   - Click "Generate new token (classic)"
   - Select scopes: `repo` (Full control of private repositories)
   - Generate and copy the token

2. Import rulesets:

```bash
# Set your token
export GITHUB_TOKEN="your_token_here"

# Import main branch ruleset
curl -X POST \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  https://api.github.com/repos/leonidbkh/tierflow/rulesets \
  -d @.github/rulesets/protect-main.json

# Import develop branch ruleset
curl -X POST \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  https://api.github.com/repos/leonidbkh/tierflow/rulesets \
  -d @.github/rulesets/protect-develop.json
```

### Method 3: Manual Setup via GitHub UI

If you prefer to set up manually:

1. Go to: https://github.com/leonidbkh/tierflow/settings/rules
2. Click "New ruleset" → "New branch ruleset"
3. Use the JSON files as reference for configuration

**Settings to configure:**

For both `main` and `develop` branches:

- **Name**: "Protect main branch" / "Protect develop branch"
- **Target branches**: `main` / `develop`
- **Rules**:
  - ✅ Restrict deletions
  - ✅ Block force pushes
  - ✅ Require pull request before merging (0 approvals)
  - ✅ Require status checks to pass:
    - `test` (Test job)
    - `clippy` (Clippy linting)
    - `fmt` (Format check)
  - ✅ Require branches to be up to date before merging

## What These Rules Do

### Protection Rules

1. **Restrict deletions** - Prevents accidental deletion of protected branches
2. **Block force pushes** - Prevents rewriting history on protected branches
3. **Require pull requests** - All changes must go through PR workflow
4. **Require status checks** - CI must pass before merging

### Required Status Checks

Based on `.github/workflows/ci.yml`:

- **test** - Runs `cargo test --all --verbose` on Ubuntu
- **clippy** - Runs `cargo clippy --all-targets -- -D warnings`
- **fmt** - Runs `cargo fmt --all -- --check`

All checks must pass before a PR can be merged.

## Verifying Rulesets

After importing, verify they're active:

```bash
# List all rulesets
gh api repos/leonidbkh/tierflow/rulesets | jq '.[] | {id, name, enforcement}'

# Or visit:
# https://github.com/leonidbkh/tierflow/settings/rules
```

## Updating Rulesets

To update an existing ruleset:

```bash
# Get ruleset ID
RULESET_ID=$(gh api repos/leonidbkh/tierflow/rulesets | jq '.[] | select(.name=="Protect main branch") | .id')

# Update the ruleset
gh api repos/leonidbkh/tierflow/rulesets/$RULESET_ID \
  --method PUT \
  --input .github/rulesets/protect-main.json
```

## Deleting Rulesets

To remove a ruleset:

```bash
# Get ruleset ID
RULESET_ID=$(gh api repos/leonidbkh/tierflow/rulesets | jq '.[] | select(.name=="Protect main branch") | .id')

# Delete the ruleset
gh api repos/leonidbkh/tierflow/rulesets/$RULESET_ID --method DELETE
```

## Troubleshooting

### Error: "Resource not accessible by personal access token"

Your token needs the `repo` scope. Create a new token with full repository access.

### Error: "Validation failed"

The JSON structure might be incorrect. GitHub's ruleset API is strict about the format. Check:
- All required fields are present
- `integration_id` is `null` (not a string)
- `ref_name` uses full refs path: `refs/heads/main`

### CI Checks Not Running

Make sure:
1. Workflows are enabled: https://github.com/leonidbkh/tierflow/actions
2. The CI workflow triggers on PRs to the protected branch
3. Job names in ruleset match exactly the job names in `.github/workflows/ci.yml`

## Workflow After Protection

Once rulesets are active:

```bash
# ❌ This will fail (protected branch)
git push origin main

# ✅ This is the correct workflow
git checkout -b feature/my-feature
git commit -am "feat: add feature"
git push -u origin feature/my-feature
# Then create PR on GitHub
```

## References

- [GitHub Rulesets API](https://docs.github.com/en/rest/repos/rules)
- [GitHub CLI Reference](https://cli.github.com/manual/)
