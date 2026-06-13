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
  - [-] Lighting
    - [-] Phase 1 — Forward Lighting (no shadows)
      - [x] CPU: define `#[repr(C)]` `GpuLight` struct (type discriminant, position, direction, color, intensity, radius, angle, length)
      - [x] CPU: create light SSBO in `VulkanRenderer` (MAX_LIGHTS=32 + count header)
      - [x] CPU: add light descriptor set layout (set 1, binding 0, `STORAGE_BUFFER`, fragment stage)
      - [ ] CPU: update all three pipeline layouts (model, voxel, water) to include the light descriptor set
      - [ ] CPU: add `set_lights(lights: &[GpuLight])` on `RenderingAPI`; call `cmd_bind_descriptor_sets` before draws
      - [ ] CPU: add system that queries `(Light, Transform)` each frame, packs `Vec<GpuLight>`, calls `set_lights`
      - [ ] Shader: define `GpuLight` struct + `LightBuffer` SSBO in GLSL (shared include or per-shader)
      - [ ] Shader: `shader.vert` — confirm `fragWorldPos` and `fragWorldNormal` are world-space (already close)
      - [ ] Shader: `shader.frag` — replace hardcoded light with loop; Lambert diffuse + Blinn-Phong specular + ambient term
      - [ ] Shader: `voxel.vert` — output `fragWorldPos` (x+world_pos) and `fragWorldNormal` (derive from face index 0-5 → ±X/Y/Z)
      - [ ] Shader: `voxel.frag` — add light loop after texture fetch; multiply with AO; keep face shading as ambient hint
      - [ ] Shader: `water.vert`/`water.frag` — same treatment as voxel
      - [ ] Shader: implement per-type light dispatch in GLSL
        - [ ] Directional (type 0): use direction, no attenuation (sun)
        - [ ] Point (type 1): direction to light, quadratic attenuation by radius
        - [ ] Spot (type 2): cone angle check + attenuation
      - [ ] Recompile all 6 shaders to `.spv` with `glslc`
    - [ ] Phase 2 — Shadow maps (directional/sun)
      - [ ] Create depth-only render pass + pipeline for shadow casting
      - [ ] Create `2048×2048` depth image + sampler with `compareEnable` for PCF
      - [ ] Render all opaque geometry from the directional light's perspective before main pass
      - [ ] Compute light-space matrix from the Directional light entity's Transform
      - [ ] Pass `lightSpaceMatrix` to fragment shaders
      - [ ] Sample shadow map in frag with 3×3 PCF kernel; multiply lighting by shadow factor
      - [ ] Consider cascaded shadow maps for large voxel view distances
    - [ ] Phase 3 — Point / spot shadows
      - [ ] Point lights: render 6-face cube map depth pass per light, sample with `samplerCube`
      - [ ] Spot lights: single depth map pass, sample with `sampler2D`
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
