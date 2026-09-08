# Offline viewer

The Python CLI and `sldkit.viewer` write a single HTML containing the viewer,
display data, and available saved images. Open it directly from disk; there are
no network requests, external fonts, CDN imports, or server requirements. A
WebGL2-capable browser is required for 3D. Images and analysis information remain
accessible when WebGL2 is unavailable.

```sh
sldkit view part.SLDPRT --output preview.html
sldkit view drawing.SLDDRW --output drawing.html --open
sldkit view part.SLDPRT --limits service --force
```

The default output replaces the input's suffix with `.html`. Existing files
require `--force`, and the source file cannot be overwritten. `--open` requests
the system browser; otherwise the command only writes the file and prints its
path. Exit 0 means an HTML was written, including for empty/unsupported parse
results; it does not certify geometry completeness. Output or input I/O errors
and invalid options return 2. The Rust CLI does not provide the viewer command.

## Python

```python
import sldkit
from sldkit.viewer import view_file, write_html

# Parse metadata, decode Part geometry, and extract verified saved previews.
view_file("part.SLDPRT", "part.html", profile="service")

# Reuse an existing result without reparsing or accessing the source file.
result = sldkit.decode_geometry_file("part.SLDPRT")
write_html(result, "geometry.html", title="Parsed part")
```

Both return the output `pathlib.Path`, accept `force=False`, and never open a
browser. `write_html` contains geometry and its diagnostics only; saved images
and document metadata are obtained by `view_file`.

## Display behavior

Saved DisplayLists meshes can be viewed even when B-Rep decoding is incomplete.
The result stays `partial`; Analysis details distinguish the B-Rep failure from
successful display-cache transfer. Body, face, and configuration relationships
remain unresolved where the parser cannot establish them.

- Orbit by dragging, zoom by scrolling, and pan with the right mouse button.
- Fit frames visible meshes. Front (+Z), top (+Y), right (+X), and isometric
  views retain source axis directions. Units come from `GeometryDocument`.
- Meshes can be toggled individually. Explicit source visibility is honored;
  unknown visibility is displayed initially. Show all restores every mesh.
- Configuration filtering is enabled only when every displayed mesh references
  a known body and the selected configuration has resolved body membership.
  Unresolved membership is disabled; resolved empty membership shows no meshes.
  The initial view shows all collected meshes, not an inferred active state.
- Source vertex/corner normals are retained. Missing normals use flat display
  shading. Source mesh color is used when present; other colors are display
  defaults. Both sides of sheet triangles are visible.
- Wireframe shows triangle boundaries. Feature edges show only the explicitly
  returned feature-edge indices. Neither establishes a complete CAD edge model.
- Source coordinates remain in the embedded data. GPU positions are rebased
  around the model center for display precision; no unit conversion is repeated.
- Saved document, configuration, and sheet images have their own tab. PNG and
  bounded Windows DIB headers 40/108/124 with BI_RGB, 16/32-bit BI_BITFIELDS,
  or bottom-up BI_RLE4/BI_RLE8 with a declared image size are supported.
  DIB bytes receive a BMP file header for browser display.
  V5 embedded/linked color profiles and other image layouts are reported and
  omitted. With no mesh, available saved
  images are shown initially; without either, an explanatory page is generated.
- Assemblies expose decoded references and saved images. Drawings expose
  sheet/view lists and saved images. References are not opened or traversed.

These are partial parser results. A mesh does not establish complete geometry,
resolved face ownership, or the native solid count. Saved images may represent
a different saved configuration and do not validate the displayed mesh.
The viewer does not reconstruct B-Rep surfaces, heal geometry, infer assembly
placement, or redraw Drawing entities/dimensions. Parser diagnostics and losses
remain in Analysis details; display omissions are separate viewer messages.

## Limits and distribution

The viewer caps displayed input at 2,000,000 vertices and 2,000,000 triangles,
64 saved images, 32 MiB of extracted image bytes, 16,777,216 pixels per image,
and 128 MiB of serialized scene JSON. Meshes with invalid coordinates/indices
or beyond the mesh budget are omitted with a message. Invalid optional normals
and feature edges are omitted independently. An oversized JSON fails before
opening the output. Parser resource limits apply independently.

The HTML contains geometry, preview images, names, decoded reference paths, and
diagnostics from the input. These data are embedded for offline viewing and
travel with the HTML. Binary geometry records and source-file payloads are not
copied wholesale.

## Developing the browser assets

The checked-in bundle is generated from `viewer/src/viewer.js`. Node.js is a
development-only requirement. Dependencies are pinned in `viewer/package-lock.json`.

```sh
npm ci --prefix viewer
npm run build --prefix viewer
npm test --prefix viewer
```

Browser tests use the repository `.venv/bin/python` and an installed Chromium.
Set `SLDKIT_TEST_PYTHON` and `SLDKIT_TEST_CHROMIUM` to override their paths.
The tests open generated files directly with networking disabled.

The build embeds the Three.js MIT notice in the bundle and HTML, and copies it
to `LICENSES/three-MIT.txt`. Wheels and sdists include all display assets, so
installing or rebuilding a Python distribution does not require npm.

Rendering uses Three.js [BufferGeometry](https://threejs.org/docs/pages/BufferGeometry.html)
and [OrbitControls](https://threejs.org/docs/pages/OrbitControls.html). The asset
build uses [esbuild](https://esbuild.github.io/getting-started/).
DIB wrapping follows Microsoft's
[BITMAPINFOHEADER](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapinfoheader)
and [BITMAPFILEHEADER](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapfileheader)
layouts; the original palette and pixel bytes are retained.
