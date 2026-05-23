# Side-by-side preview

Click any thumbnail to open the preview modal. The left pane shows the source;
the right pane shows the result of applying your current settings.

## Zoom and pan

- **Scroll** to zoom in and out.
- **Drag** to pan.
- The two panes share zoom and pan state, so they always show the same
  region of the image — useful for spotting compression artifacts or
  sharpness loss after resize.

## Caveats

- **Crop preview**: when crop is active, the right pane shows the cropped
  result. The two panes are still synced by normalized coordinates, so the
  "same point" in each pane corresponds to the same point in screen space,
  not necessarily the same point in image space.
- The preview re-renders whenever you change a setting in the side panel,
  so you can dial in a target file size or aspect ratio interactively
  without running a full batch.
- For very large images, the first preview render can take a moment — the
  source is decoded fresh each time the modal opens.
