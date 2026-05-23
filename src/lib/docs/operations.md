# Resize, crop, rotate

Each operation can be turned off independently. The pipeline order is fixed:
**rotate → crop → resize → encode**. That order matters because rotating first
means crop and resize work on the upright image, not the sensor orientation.

## Resize

- **None** — keep original dimensions.
- **Long edge ≤** _N_ px — shrink so the longer side is at most _N_ pixels.
  Aspect ratio is preserved. Images already smaller than _N_ are left alone.
- **Percentage** — scale both sides by the same factor.

Resizing uses `fast_image_resize` with a Lanczos3 filter — good quality for
photographs, no visible aliasing.

## Crop

- **None** — keep the full frame.
- **Aspect ratio** — center-crop to the given ratio (e.g. 16:9, 1:1, 4:5).
  The largest rectangle of that ratio that fits inside the source is used.
- **Pixels** — center-crop to a specific width × height. If the source is
  smaller than the requested crop, the source dimensions are used instead
  (no upscaling).

## Rotate / flip

A single choice from: none, 90° CW, 180°, 270° CW, flip horizontal, flip
vertical. EXIF orientation from the source is applied **before** this user
rotation, so a sideways portrait from a phone first becomes upright, then
your chosen rotation is applied on top.

## Output format

- **Keep original** — same format as the source.
- **JPEG**, **PNG**, **WebP** — convert to that format on output.

HEIC and AVIF inputs are always converted to JPEG on output. There's no HEIC
encoder shipped, and AVIF encoding isn't in scope yet.

## Compression

- **Quality (JPEG/WebP)** — a 0–100 slider. WebP at quality 100 is still
  lossy; use the "lossless" escape hatch in stored settings if you really
  need it.
- **Target file size** — give a kilobyte budget and Pixmill binary-searches
  on encoder quality to land under it. Only works for JPEG and WebP. PNG
  doesn't have a quality knob, so target-size mode falls back to default
  encode for PNG outputs.
