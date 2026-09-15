# Asset locations

Reusable Starframe logos, wordmarks and finished brand exports live outside the repository in the sibling `Starframe Assets` directory. Third-party originals and source records live in the sibling `Sanctuary Artwork` directory. Each has a local HTML index. Keep originals and provenance records when producing a derivative.

The application imports its unchanged launch background from [sanctuary-sphere.jpg](../../src/assets/sanctuary-sphere.jpg). [The artwork notice](../notices/Sanctuary-artwork.md) records its source, hash and the owner-supplied FoneE permission conditions. Its CSS crop/fade belongs to the launch control; the original is not modified. The external artwork collection now includes the same permission record and links to it from its index.

Starframe’s original SVG marks remain under this directory. Third-party artwork is separate from the repository’s AGPL code license and must retain its credit and use conditions.

The sidebar wordmark and launch button use [Oxanium](../../src/assets/Oxanium.ttf), weight 800, from the external `Starframe Assets/Font` source used by the social preview. The unchanged variable font is bundled under the [SIL Open Font License](../notices/OFL-Oxanium.txt). The sidebar spells STARFRAME in capitals. The launch label has no separate background rectangle.

## Windows installer artwork

The [installer sidebar](../../src-tauri/windows/assets/installer-sidebar.bmp) adapts the existing `Starframe Assets/Social preview/starframe-social-v1.png` into a portrait composition. Image generation produced the derivative on 15 September 2026; the original social preview and original SVG identity remain unchanged. The instruction retained the frame-and-sun mark, uppercase wordmark, navy space and constructed sphere, moved the identity above the sphere, and removed the small subtitle. Raster lettering is an interpretation of the source, not a replacement for the original outlined Oxanium exports.

The generated PNG and 24-bit BMP are 907 × 1734. NSIS preserves their aspect ratio; welcome and completion text moves to the right of the actual bitmap edge. The reusable PNG/BMP exports and provenance are retained under `Starframe Assets/Installer`, linked from that collection's index.

- PNG SHA-256: `ae323eb685bca74a8db49e23cf5e2f7fe1c4226ae2d5923920eb954222e17da2`.
- BMP SHA-256: `11a001b6e669f48e85b55f83a8c113067bf85ac1c9de0503b52e882ae96f7ffb`.

The [Sanctuary artwork notice](../notices/Sanctuary-artwork.md) also applies to this derivative and is included in the installer. Installer controls use Windows' installed Segoe UI; headings use Bahnschrift when registered and Segoe UI otherwise. No Windows font files are redistributed.
