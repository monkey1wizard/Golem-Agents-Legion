---
name: setup-browser-cookies
description: "Import real browser cookies from Chrome, Arc, Brave, Edge, or Comet into the active Playwright session. Enables testing authenticated pages. Cookies are in-memory only — never written to disk."
---

# /setup-browser-cookies

Import real browser cookies into the Playwright session so you can test authenticated pages.

## Role

Session bootstrapper. Get real auth into the test browser without asking the user to type credentials.

## When to Use

- Before `/qa` on a page that requires login
- Before `/design-review` on a page behind authentication
- When manual login in the browser would be faster than coding a login flow

## Security Rules

1. **Never write cookies to disk.** Load into memory only.
2. **Never log cookie values.** You may log cookie names and domains.
3. **Never send cookies to external services.** Only use in the local Playwright session.

## Step 1 — Ask Which Browser

Ask the user: "Which browser should I import cookies from? Chrome / Arc / Brave / Edge / Comet / Other"

If the user says "Other": ask for the path to the cookies SQLite file.

## Step 2 — Ask Which Domain

Ask: "Which domains? (e.g. `localhost:3000, app.example.com`) Or press enter for all domains."

## Step 3 — Locate the Cookies File

Default cookie file paths by browser:

| Browser | Platform | Path |
| --- | --- | --- |
| Chrome | macOS | `~/Library/Application Support/Google/Chrome/Default/Cookies` |
| Chrome | Windows | `%LOCALAPPDATA%\Google\Chrome\User Data\Default\Cookies` |
| Arc | macOS | `~/Library/Application Support/Arc/User Data/Default/Cookies` |
| Brave | macOS | `~/Library/Application Support/BraveSoftware/Brave-Browser/Default/Cookies` |
| Edge | Windows | `%LOCALAPPDATA%\Microsoft\Edge\User Data\Default\Cookies` |

Chrome-family browsers encrypt the cookie value. On macOS, the key is in Keychain (`Chrome Safe Storage`). On Windows, DPAPI is used.

## Step 4 — Decrypt and Load

On macOS:
1. Read the encryption key from Keychain using the `security` CLI
2. Decrypt cookie values with AES-128-CBC, key = PBKDF2(keychain_key, b'saltysalt', 1003, 16)
3. Filter to requested domains
4. Load into Playwright via `context.addCookies()`

On Windows:
1. Read the encrypted key from `Local State` JSON
2. Decrypt with DPAPI via PowerShell `[System.Security.Cryptography.ProtectedData]::Unprotect()`
3. Use decrypted key as AES-256-GCM key for cookie values
4. Filter to requested domains
5. Load into Playwright via `context.addCookies()`

## Step 5 — Verify

Navigate to the first filtered domain. Check that the session is authenticated by looking for a user-specific element (e.g. avatar, name, dashboard link).

Tell the user: "Loaded N cookies for [domains]. Session appears authenticated." (or "Session did not appear authenticated — manual login may be required.")

## No Plan Artifacts

`/setup-browser-cookies` does not write to the plan file.
