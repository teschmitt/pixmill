# Troubleshooting

## "Unidentified developer" warning on macOS

Pixmill release builds aren't code-signed yet. On first launch, right-click
the app in Finder and choose **Open** to get past the Gatekeeper dialog.
After that, double-clicking works normally. Same dialog can also be cleared
from **System Settings → Privacy & Security**.

## Windows SmartScreen warning

Same root cause — bundles aren't signed. Click **More info → Run anyway** on
the SmartScreen dialog.

## HEIC or AVIF files fail to process

These formats need optional system libraries (`libheif` for HEIC, `dav1d`
for AVIF) and a build with the corresponding Cargo features enabled. The
default release bundles don't include them.

If you built from source, build with:

```sh
pnpm tauri build -- --features "pixmill-core/avif-decode pixmill-core/heic"
```

after installing the system libs (`brew install dav1d libheif` on macOS,
`apt install libdav1d-dev libheif-dev` on Linux).

## "Output folder is the same as a source folder"

Pixmill refuses to write into a folder that overlaps with a source folder or
a watch folder. Pick a different destination. This is a guardrail against
clobbering originals.

## Files queued but Run does nothing

Check that you've picked an output folder in the right-hand panel. The Run
button is disabled until both the queue and the output folder are set.

## Watch folder shows an error

The path probably became unreachable. Click **Retry now** on the row, or
wait — a background task retries error folders every 30 seconds. On Linux,
errors after watching many recursive folders are usually inotify-limit
exhaustion — bump it with `sysctl fs.inotify.max_user_watches=524288`.

## Where do my settings live?

In a JSON file under your user config dir:

- **macOS** — `~/Library/Application Support/com.thomasschmitt.pixmill/`
- **Windows** — `%APPDATA%\com.thomasschmitt.pixmill\`
- **Linux** — `~/.config/com.thomasschmitt.pixmill/`

Deleting that folder resets Pixmill to defaults.
