# Getting started

Pixmill batch-processes a pile of images using whatever resize, crop, rotate,
and format settings you pick. Your originals are never touched — output goes
to a folder you choose.

## The basic flow

1. **Add images.** Drag-and-drop files or folders onto the drop zone, or use
   the file/folder picker. Recursive folder ingest is supported.
2. **Pick an output folder.** Use the "Output folder" control in the right-hand
   panel. Pixmill will refuse to write into the same folder your sources live
   in — it always writes to a separate destination.
3. **Set your operations.** Resize, crop, rotate, output format, and
   compression are all in the right-hand settings panel. Defaults are sticky,
   so the next launch starts where you left off.
4. **Hit Run.** The progress bar streams per-file status. You can click any
   thumbnail to open the side-by-side preview while a batch is running or
   after it finishes.

## Tips

- The queue holds up to ~100 thumbnails comfortably. Larger queues still work,
  but the thumbnail grid is the bottleneck — processing isn't.
- "Pending" rows can be removed by hovering and clicking the × badge. "Done"
  and "error" rows stay until you clear the queue.
- The output folder, last-used settings, and any watch folders are saved
  automatically. Nothing is uploaded anywhere — everything lives in your
  user data directory.
