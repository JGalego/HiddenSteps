`tauri.conf.json`'s `bundle.icon` lists the generated platform set below; `app.trayIcon` uses `icon.png` directly.

## The mark

Five stepping stones on a rising, gently curved path — most of them a quiet teal, the last one lit warm amber with a soft glow. That's the product in one glyph: HiddenSteps watches an ordinary, repeating sequence of steps and quietly points at the one you hadn't noticed — the step that could disappear. Flat, two-tone, no gradients or gloss, dark slate background — reads clearly from a 16px tray icon up to a 512px app icon, and doesn't compete with the OS's own icon style.

The source (`icon.png`, 512×512) is generated programmatically (Pillow, supersampled 4x and downsampled for clean anti-aliasing) rather than drawn in a design tool — reproducible and easy to re-tune (spacing, colors, glow radius are all named constants), but a professional pass in a real design tool before shipping this widely would still be worth doing.

## The generated set

Everything else in this directory (`icon.ico`, `icon.icns`, `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`) was produced from `icon.png` via the real `tauri icon` CLI (not hand-rolled) — this is what closed the "`icons/icon.ico` not found; required for generating a Windows Resource file" build failure CI's first real Windows compile surfaced. `tauri icon` also generates iOS/Android asset sets and Windows Store tile logos by default; those were deleted here since this is a desktop-only app with no mobile or Store target configured.

Regenerate everything from a new source image with:

```sh
cargo tauri icon icons/icon.png -o icons
# then delete the ios/, android/, and Square*Logo.png/StoreLogo.png files it
# also creates, per the note above
```
