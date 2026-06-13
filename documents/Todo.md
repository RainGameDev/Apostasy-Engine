# Engine

- [x] World
  - [x] Ability to access renderer in systems
  - [x] Manual system registration
  - [x] Scene hierarchy (parent/child objects)
  - [x] Tags system
  - [x] States system

- [-] Rendering
  - [x] Render settings
  - [x] Anti-aliasing
  - [x] Frustum culling
  - [x] Shader registry and loader
  - [x] GLTF / model loading
  - [ ] Lighting
    - [ ] Shadow maps
    - [ ] Point lights
    - [ ] Directional lights
    - [ ] Spot lights
  - [ ] Ambient effects
    - [ ] Ambient lighting
    - [ ] Fog
  - [ ] Particle system

- [-] Systems
  - [x] Prerender systems
  - [ ] Render systems

- [x] Physics
  - [x] Basic collisions
    - [x] Object resolution
    - [x] Object shapes
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
    - [x] Objects displaying
    - [x] Object selection
    - [x] Object deletion
    - [x] Object creation
  - [-] Object Editor
    - [x] Name bar (inline rename)
    - [x] Component adding
    - [x] Component editing
    - [x] Component removal
    - [x] Component copy/paste/cut
    - [x] Tag adding
    - [x] Tag removal
  - [x] Assets panel (loaded assets display)
  - [x] Viewport panel
  - [x] Cell / Objects panel
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

- [-] Editor Camera
  - [x] Camera rotation
  - [x] Object focusing
  - [x] Camera speed
  - [ ] Save the cameras position and rotate

- [x] Editor systems
  - [x] Copy/paste objects (keybinds)
  - [x] Raycasting (viewport object selection)
  - [x] Layout saving and loading

- [x] Viewport Gizmos
  - [x] Translate handle
  - [x] Rotate handle
  - [x] Scale handle
  - [x] Hover highlighting
  - [x] Global / Local space mode

- [ ] Play mode
  - [ ] Play / Pause / Stop
  - [ ] Switch between editor and game camera

- [x] Undo / Redo

# Apostasy
