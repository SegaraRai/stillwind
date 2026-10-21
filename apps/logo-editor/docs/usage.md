# Shape Lab user guide

## Editing

- **Middle-click:** delete the pointed vertex and reconnect its neighbors, or delete the pointed edge. Point hits take priority. Middle-drag away from points and edges pans the canvas; Space + left-drag also pans.
- **Double-click:** add a point in any tool, including insertion on an edge. Existing points are reused.
- **Shift-click:** toggle point selection. With at least two points selected, **Shift-double-click a selected point** adds edges from that point to all other selected points. Existing edges are retained. The complete operation is one undo step.

- **V — Select / move:** drag a shared point; every incident face follows. Shift-click selects multiple points. The inspector exposes exact coordinates and point width. Delete removes selected points and reconnects their angularly ordered neighbors, or removes the selected edge. A degree-two vertex becomes one edge; deleting a corner replaces it with a straight chord.
- **P — Add point:** click inside the 480 × 480 artboard. Clicking an edge inserts a shared point and interpolates its width along that edge.
- **L — Connect / disconnect:** click two points. Crossings become shared vertices automatically. Selecting the same endpoints again removes the existing straight connection, including segments separated by intersection points.
- **F — Face:** choose a face to change its color or hide it without deleting topology. Faces can also be selected from the inspector list.
- Position assist snaps to shared horizontal/vertical coordinates, existing edge directions, standard angles, and the fourth corner of a parallelogram defined by two connected sides. Hold Shift while dragging to bypass snapping.
- Wheel zooms around the pointer; Space-drag pans. The view reset control restores the full view. Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z undo and redo document edits.

Both presets start from a complete square. The default includes partitions; the unpartitioned preset has a single square face. The upper-right and lower-left corners remain editable faces: hide them through the face inspector when needed. Switching presets is undoable. JSON saves the shared graph, widths, and face colors; importing JSON is also undoable. The current document is saved locally in the browser when a change is committed. Existing saved drawings are restored without resetting them to a preset. SVG export contains only generated filled geometry, without editor handles or a background.

## Transform

Use the controls for rotating 90° left or right and flipping horizontally or vertically to transform the whole drawing about the canvas center (240, 240). Shared topology, point widths, colors, and visibility follow the geometry. Each operation is one undo step.

## Grid

The canvas is 480 × 480 units with a centered 360 × 360 design region, from (60, 60) to (420, 420). Grid divisions are measured across this region: 24 divisions means 15-unit spacing. Both endpoints are grid lines, including for counts that do not divide 360 evenly. The initial preset vertices use this same 15-unit lattice.

Choose a shared division count for both axes, independent X/Y counts, or no grid. Counts are integers from 1 to 360; in independent mode, 0 disables that axis. Changing these settings never moves existing points. When both grid coordinates are within snapping distance, the grid intersection takes precedence over parallelogram completion and vertex alignment. A match on only one axis keeps the existing parallelogram priority. Grid lines and snapping share the design-region origin. Grid lines are drawn over face fills and below editing handles. The bottom grid visibility checkbox toggles visibility only. Settings are saved locally, separately from drawing JSON.

## Width and geometry

Point width is measured in the 480-unit artboard coordinate system. Each edge offsets its two incident faces by half the widths of its endpoints. With equal endpoint widths, the perpendicular gap is constant. Different endpoint widths produce a taper confined to that edge segment. Intermediate collinear points can therefore vary the gap locally without extending their slope across other segments. Apply the width to all points for uniform spacing. Each point width is saved in the document. The common width used for newly added points is saved separately and restored after reload; it does not overwrite existing point widths. When no common-width setting exists, a document with uniform point widths supplies the initial common width.

The graph remains the editable source of truth. Rendered inset corners are computed intersections of offset boundary lines; they are not independently draggable vertices. Triangle/parallelogram labels describe the graph face before inset. Assist is a placement aid, not a persistent constraint solver: moving another point does not enforce previously snapped relationships.

Convex and concave faces of any corner count can be colored. Collinear points retain their graph connections and widths but are excluded from the corner count and corner list. Fills subtract finite boundary strips with miter joins; inset components and holes are rendered identically in the canvas, previews, and SVG. A narrow neck can separate into multiple filled components. If width collapses a face, the editor marks it as too wide. SVG export rejects visible invalid faces rather than silently omitting them. Disconnected nested contours are not a compound-path/hole editing workflow; use connected partitions and hidden faces for cutouts. Crossing or overlapping edges are rejected during a point drag; add new crossing edges with the connect tool instead. Documents are limited to 300 shared points.
