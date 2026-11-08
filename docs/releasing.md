# Releasing the Mac app

Pushing a tag like `v0.2.0` runs the Release workflow
(`.github/workflows/release.yml`) on a Mac runner. It builds one Sticks.app
for Apple Silicon and Intel, signs it with your Developer ID, has Apple
notarize it, and publishes a GitHub release with:

- `Sticks_<version>_universal.dmg`: what people download and install.
- `Sticks.app.tar.gz` and its `.sig`: the update the app downloads.
- `latest.json`: what installed copies read to learn a new version is out.

Copies installed from the .dmg check `latest.json` on every start. When a
newer version is out they show a card, and **Install and restart** downloads
it, checks its signature against the public key built into the app, replaces
the app and restarts. Copies built from source (`npm run app:install`) still
show the `git pull` card, and the App Store build checks nothing.

## One-time setup

Everything below goes into the repository's Actions settings:
[Secrets](https://github.com/wbgoodwin/sticks/settings/secrets/actions) and,
for the one public value, the **Variables** tab next to it. With the
[GitHub CLI](https://cli.github.com) you can instead pipe each value into
`gh secret set NAME` (or `gh variable set NAME`) from the repository folder,
which keeps them off the clipboard. You need a Mac with Xcode installed and to
be the Account Holder of the Apple Developer account (only the Account Holder
can make Developer ID certificates).

### 1. Developer ID certificate (code signing)

1. Open Xcode, then **Settings → Accounts**, pick your Apple ID, click
   **Manage Certificates…**, click **+** and choose **Developer ID
   Application**. (Already have one? Skip to the next step.)
2. Open **Keychain Access**, go to **login → My Certificates**, right-click
   **Developer ID Application: <your name> (<team id>)** and choose
   **Export…**. Save it as `sticks.p12` and set a password.
3. Add the secrets:

   ```sh
   base64 -i sticks.p12 | gh secret set APPLE_CERTIFICATE
   gh secret set APPLE_CERTIFICATE_PASSWORD     # paste the export password
   security find-identity -v -p codesigning     # copy the quoted name
   gh secret set APPLE_SIGNING_IDENTITY         # paste it, e.g. Developer ID Application: Will Goodwin (AB12CD34EF)
   ```

4. Delete `sticks.p12` once the secrets are set.

### 2. Notarization (Apple ID)

1. At [account.apple.com](https://account.apple.com), go to **Sign-In and
   Security → App-Specific Passwords**, make one called "Sticks notarize" and
   copy it.
2. Find your Team ID: the 10 characters in brackets in the signing identity
   above, or **Membership details** at
   [developer.apple.com/account](https://developer.apple.com/account).
3. Add the secrets:

   ```sh
   gh secret set APPLE_ID          # your Apple ID email
   gh secret set APPLE_PASSWORD    # the app-specific password
   gh secret set APPLE_TEAM_ID     # e.g. AB12CD34EF
   ```

### 3. Update signing key

The app only installs updates signed with this key. **Keep a backup of the
private key and its password** (in your password manager): if it's lost,
copies already installed can't update and people have to download the .dmg
again.

```sh
cd app
npx tauri signer generate -w ~/.tauri/sticks-updater.key   # asks for a password
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/sticks-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD             # paste that password
gh variable set TAURI_UPDATER_PUBKEY < ~/.tauri/sticks-updater.key.pub
```

### 4. Sticks accounts (optional)

To ship the release with Sign in turned on, set these **variables** to the
sticks-yard stack outputs. Leave them unset to ship it without accounts.

```sh
gh variable set VITE_YARD_URL --body https://api.sticksyard.app
gh variable set VITE_YARD_SIGN_IN --body https://sticks-yard.auth.us-east-1.amazoncognito.com
gh variable set VITE_YARD_CLIENT_ID --body <ClientId>
```

If any secret is missing, the workflow stops at its first step and names it.

## Each release

1. Raise `version` in `app/src-tauri/tauri.conf.json` (the app, the tag and
   the update all use this one), and merge it to `main`.
2. Tag that commit and push the tag:

   ```sh
   git checkout main && git pull
   git tag v0.2.0 && git push origin v0.2.0
   ```

3. Watch the run on the [Actions tab](https://github.com/wbgoodwin/sticks/actions/workflows/release.yml).
   Notarizing usually takes a few minutes, now and then longer.

The release is public as soon as the run finishes, and installed copies offer
it on their next start.

## Checking a build

After downloading the .dmg and dragging Sticks to Applications:

```sh
codesign --verify --deep --strict --verbose=2 /Applications/Sticks.app
spctl --assess --type execute --verbose /Applications/Sticks.app   # "accepted, source=Notarized Developer ID"
xcrun stapler validate /Applications/Sticks.app
```

## The App Store build

The App Store build (see the Mac App Store plan) builds with
`VITE_CHANNEL=appstore` and without `tauri.release.conf.json`, so it has no
updater: the App Store updates it.
