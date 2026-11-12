## NyashkPre 1.0.2

- **Alt-tab fix (Linux):** after alt-tab, minimising or covering the window, parts of the picture (backgrounds, knobs, images) could stay blank until something on them moved. The window is now drawn again in full whenever the system throws its contents away, and pictures are no longer lost while the window is hidden or a frame is skipped.
- Smoother images in the CPU renderer (used when Vulkan is not available): pictures drawn smaller than their real size, such as knobs and photos, are now scaled with mipmaps instead of picking single pixels, so they no longer look grainy or jagged.
- **Display fix:** the built-in CPU renderer (used when Vulkan is not available) drew pixels on the shared edges of triangles twice, which showed as brighter seams and specks in see-through fills (spectra, curves, bars). It now uses an exact fill rule like a GPU: every pixel is drawn once, without holes.
- The "harmonics vs signal" label no longer runs into the OUT meter.
- Value boxes have a small gap between them, and a value that would not fit its box is drawn smaller instead of running over the frame.
- Clicking a value box and pressing Enter no longer changes a value whose unit matters (for example "1.03 s" was read back as 1.03 ms).
- No more "-0.0" read-outs. (Warmth, Presence, Silk)
- New automatic test: every parameter's text is typed back over its whole range and must give the same value.

## NyashkPre 1.0.1

- **Live spectrum** behind the tone curve.
- **Live trace on the saturation curve:** the newest samples light up on the curve, so you see how hard the preamp is being driven.

## NyashkPre 1.0.0

First release. VST3 for Linux, Windows and macOS.
