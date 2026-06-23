# Engine

- [x] World
  - [x] Ability to access renderer in systems
  - [x] Manual system registration
  - [x] Scene hierarchy (parent/child entities)
  - [x] Tags system
  - [x] States system

- [x] Rendering
  - [x] Render settings
  - [x] Anti-aliasing
  - [x] Frustum culling
  - [x] Shader registry and loader
  - [x] GLTF / model loading
  - [x] Lighting
    - [x] Phase 1 - Forward Lighting (no shadows)
    - [x] Phase 2 - Shadow maps (directional/sun)
    - [x] Phase 3 - Point / spot shadows
    - [ ] Ambient effects
      - [ ] Ambient lighting
      - [ ] Fog
  - [ ] Particle system
  - [x] GLTF loading textures
  - [ ] Custom material shaders

- [-] Systems
  - [x] Prerender systems
  - [ ] Render systems

- [x] Physics
  - [x] Basic collisions
    - [x] Entity resolution
    - [x] Entity shapes
    - [x] Offsets
  - [x] Friction
  - [x] Inertia
  - [x] Raycasting

- [x] Scene serialization
  - [x] Save scene to file
  - [x] Load scene from file

- [ ] Audio system

# Editor

- [-] Editor UI
  - [x] Top bar menu
  - [x] Inspector
    - [x] Entities displaying
    - [x] Entity selection
    - [x] Entity deletion
    - [x] Entity creation
  - [-] Entity Editor
    - [x] Name bar (inline rename)
    - [x] Component adding
    - [x] Component editing
    - [x] Component removal
    - [x] Component copy/paste/cut
    - [x] Tag adding
    - [x] Tag removal
  - [x] Assets panel (loaded assets display)
  - [x] Viewport panel
  - [x] Cell / Entities panel
  - [x] Preferences panel
    - [x] Font loading and themes
    - [x] Camera speed setting
    - [x] Graphics settings (AA, supersampling)
    - [x] Saving and loading preferences
  - [-] Scenes panel
    - [x] Scene creation
    - [ ] Scene deletion
    - [x] Scene switching
  - [ ] UI Editor
  - [ ] Dockable Panels

- [-] Editor Camera
  - [x] Camera rotation
  - [x] Entity focusing
  - [x] Camera speed
  - [ ] Save the cameras position and rotate

- [x] Editor systems
  - [x] Copy/paste entities (keybinds)
  - [x] Raycasting (viewport entity selection)
  - [x] Layout saving and loading

- [x] Viewport Gizmos
  - [x] Translate handle
  - [x] Rotate handle
  - [x] Scale handle
  - [x] Hover highlighting
  - [x] Global / Local space mode
  - [ ] Lighting gizmo
  - [ ] Ability to drag edges of a colldier to change its size

- [ ] Play mode
  - [ ] Play / Pause / Stop
  - [ ] Switch between editor and game camera

- [x] Undo / Redo

# Apostasy
