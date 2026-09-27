# PushToTalk 05A icons

The approved mark is **05A / EMIT**: a full rounded source at lower left and two
sound lobes rising to the right. Do not add the button, concavity or horizontal
cut from the rejected later explorations. No letter monogram or microphone.

`mark.svg` is the shared vector source, traced from the approved concept in
`design/05a-approved.png`. The reconstruction has three shapes and 99.57% binary
silhouette overlap with the original at its source resolution. Edit this source,
then regenerate the platform assets; do not draw each platform's mark separately.

| Surface | Assets | Presentation |
| --- | --- | --- |
| Windows EXE, shortcuts, taskbar, installer | `icon.svg`, PNG sizes, `icon.ico` | White 05A on a near-black rounded tile |
| macOS Dock, Finder, app switcher | `icon-macos.svg`, `icon-macos.png`, `icon.icns` | Same tile with Mac-specific outer margins and optical alignment |
| macOS menu bar | `tray-template.svg`, 1x/2x PNGs | Transparent 22×18pt silhouette, tinted by AppKit in light/dark/selected states |
| Windows notification area | `tray-windows.svg`, `tray-windows.png` | Same mark with reduced padding for small sizes |

Tray variants add 16 source units between adjacent shapes before fitting the
glyph to its native size. This optical adjustment prevents the waves merging at
16px; it is not a change to the application mark. Windows ICO frames at
16/24/32px use this small-size treatment as well.

Regenerate the wrappers and all native containers from the repository root:

```sh
node scripts/convert-icon.mjs
```

The converter uses the existing Sharp, png-to-ico and Tauri CLI dependencies. Native icon
containers are generated in a temporary directory; unrelated mobile/store assets
are not written into this repository. macOS configuration overrides the Windows
icon list, and platform tray construction selects the appropriate native asset.

Visual acceptance: inspect application assets at 16/32/64/128px and the menu
template at its real 22×18pt size on light, dark and selected backgrounds. Preserve
both sound-wave gaps, transparent outer edges and a balanced Dock tile size.
Changes require rebuilding the application; updating files alone does not refresh
an already running binary's embedded tray icon.
