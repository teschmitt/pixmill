# Watch folders

A watch folder is a directory Pixmill keeps an eye on. New files dropped into
it show up in the queue automatically. If "Auto-process" is enabled for that
folder, they're also processed without you clicking Run.

Watch folders are a **desktop-only** feature. The web build doesn't have them,
because browsers can't observe a filesystem path.

## Adding a folder

1. Open the **Watch folders** panel.
2. Pick a folder.
3. Toggle **Recursive** if subfolders should also be watched.
4. Toggle **Auto-process** if you want files to flow straight through the
   pipeline as they arrive.

## How file events are handled

- **New file** → enqueued. If auto-process is on, it's batched with any
  others that arrived in the same ~250 ms window and run as a burst.
- **File modified in place** → flipped back to "pending" and re-processed
  (auto-process folders only).
- **File deleted** → removed from the queue if it was still pending.
  Already-processed entries stay so you have a record.

## When a folder breaks

If a path becomes unreadable (unplugged drive, permission change, OS file-
watcher limit exceeded), the row shows an **error** status and a "Retry now"
button. A background recovery task also retries error folders every 30
seconds automatically. The retry button has a 2-second cooldown so it can't
be spammed.

On Linux you may hit the inotify watcher limit if you watch a very large
recursive tree. Bump it with `sysctl fs.inotify.max_user_watches=524288`.
