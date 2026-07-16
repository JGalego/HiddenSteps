`tauri.conf.json` references `icons/icon.png` for the app/bundle/tray icon.

## The mark

Five stepping stones on a rising, gently curved path — most of them a quiet teal, the last one lit warm amber with a soft glow. That's the product in one glyph: HiddenSteps watches an ordinary, repeating sequence of steps and quietly points at the one you hadn't noticed — the step that could disappear. Flat, two-tone, no gradients or gloss, dark slate background — reads clearly from a 16px tray icon up to a 512px app icon, and doesn't compete with the OS's own icon style.

Generated programmatically (Pillow, supersampled 4x and downsampled for clean anti-aliasing) rather than drawn in a design tool — reproducible and easy to re-tune (spacing, colors, glow radius are all named constants), but a professional pass in a real design tool before shipping publicly would still be worth doing.

## Before a real `tauri build`

Only `icon.png` (512×512) exists. Run `tauri icon icons/icon.png` from `src-tauri/` to generate the full platform set bundling needs — `.ico` (Windows), `.icns` (macOS), and the sized PNGs for Linux — from this one source image.
