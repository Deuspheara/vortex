# Right Panel Tabs

The right panel is a tab host, not a fixed three-way selector. Mutable tab state
lives in `AgentWindow` as `InspectorTabs`; rendering stays stateless in
`features/inspector/layout.rs`.

## Model

- `InspectorTabs` owns tab order, active tab id, and id allocation.
- `InspectorTabKind::BuiltIn` covers the current review surfaces:
  `Changes`, `Plan`, and `Terminal`.
- `InspectorTabKind::Artifact` opens a tab bound to an `ArtifactId`.
- `InspectorTabKind::Custom` is the extension point for future browser, search,
  Android simulator, iOS simulator, and user-defined panels.

## Orchestration

All tab mutation belongs in `window/orchestration/inspector.rs`:

- `select_inspector_tab` activates an existing tab.
- `close_inspector_tab` removes a closeable tab and picks the next neighbor.
- `new_inspector_tab` creates a custom empty panel slot.
- `select_inspector_view` is the compatibility bridge for older call sites that
  still ask for a built-in view.

Diff/review code should keep using `open_diff_panel`, `set_review_tab`, and
`apply_diff_panel_now`; those methods now select the matching inspector tab.

## Adding A Panel Type

1. Add a variant to `InspectorCustomSlot` or use `InspectorTabKind::Artifact`
   for content tied to a runtime artifact.
2. Map the kind to an icon in `inspector_tab_icon`.
3. Render the body in `render_inspector_body`.
4. Add orchestration in `AgentWindow` to open/select the tab.

Do not put `Entity<AgentWindow>` in inspector components. Pass callbacks through
props, and keep all state changes in `AgentWindow`.
