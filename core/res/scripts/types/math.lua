---@meta

-- Math ergonomics provided by the engine prelude: vector and quaternion types,
-- plus a few scalar helpers added to the stdlib `math` table. These are plain
-- Lua tables with metatables — a vector's components live in its array part, so
-- it serializes to a `[x, y, z]` sequence and can be passed straight to
-- `World:set_component`. Reading a component back yields a plain array; wrap it
-- with `vec3(...)` to get the operators again.

-- ---------------------------------------------------------------------------
-- vec3
-- ---------------------------------------------------------------------------

---@class vec3
---@field x number
---@field y number
---@field z number
---@field [1] number
---@field [2] number
---@field [3] number
---@operator add(vec3): vec3
---@operator sub(vec3): vec3
---@operator mul(vec3|number): vec3
---@operator div(vec3|number): vec3
---@operator unm: vec3
local Vec3 = {}

---@return number
function Vec3:length() end
---@return number
function Vec3:length_squared() end
---@return vec3 unit-length copy (zero vector if length is 0)
function Vec3:normalized() end
---@param other vec3
---@return number
function Vec3:dot(other) end
---@param other vec3
---@return vec3
function Vec3:cross(other) end
---@param other vec3
---@return number
function Vec3:distance(other) end
---@param other vec3
---@param t number interpolation factor 0..1
---@return vec3
function Vec3:lerp(other, t) end
---@return vec3
function Vec3:copy() end
---@return number x, number y, number z
function Vec3:unpack() end

---Constructs a vec3. Accepts `vec3()` (zero), `vec3(n)` (splat),
---`vec3(x, y, z)`, or `vec3(table)` (from an array or `{x=,y=,z=}`).
---@class vec3lib
---@field zero vec3
---@field one vec3
---@field up vec3
---@field down vec3
---@field right vec3
---@field left vec3
---@field forward vec3
---@field back vec3
---@overload fun(): vec3
---@overload fun(splat: number): vec3
---@overload fun(t: table): vec3
---@overload fun(x: number, y: number, z: number): vec3
vec3 = {}

-- ---------------------------------------------------------------------------
-- vec2 / vec4 (same arithmetic as vec3, fewer/more components)
-- ---------------------------------------------------------------------------

---@class vec2
---@field x number
---@field y number
---@operator add(vec2): vec2
---@operator sub(vec2): vec2
---@operator mul(vec2|number): vec2
---@operator div(vec2|number): vec2
---@operator unm: vec2
local Vec2 = {}
---@return number
function Vec2:length() end
---@return vec2
function Vec2:normalized() end
---@param other vec2
---@return number
function Vec2:dot(other) end

---@class vec2lib
---@overload fun(): vec2
---@overload fun(splat: number): vec2
---@overload fun(t: table): vec2
---@overload fun(x: number, y: number): vec2
vec2 = {}

---@class vec4
---@field x number
---@field y number
---@field z number
---@field w number
---@operator add(vec4): vec4
---@operator sub(vec4): vec4
---@operator mul(vec4|number): vec4
---@operator div(vec4|number): vec4
---@operator unm: vec4
local Vec4 = {}
---@return number
function Vec4:length() end
---@return vec4
function Vec4:normalized() end

---@class vec4lib
---@overload fun(): vec4
---@overload fun(splat: number): vec4
---@overload fun(t: table): vec4
---@overload fun(x: number, y: number, z: number, w: number): vec4
vec4 = {}

-- ---------------------------------------------------------------------------
-- quat (unit quaternion; `*` composes rotations)
-- ---------------------------------------------------------------------------

---@class quat
---@field x number
---@field y number
---@field z number
---@field w number
---@operator mul(quat): quat
local Quat = {}
---@return number
function Quat:length() end
---@return quat
function Quat:normalized() end
---@return quat
function Quat:conjugate() end
---@return quat
function Quat:inverse() end
---Rotates a vec3 by this quaternion.
---@param v vec3
---@return vec3
function Quat:rotate(v) end

---@class quatlib
---@overload fun(x: number, y: number, z: number, w: number): quat
quat = {}

---@return quat the identity rotation (0, 0, 0, 1)
function quat.identity() end
---@param axis vec3
---@param degrees number
---@return quat
function quat.from_axis_angle(axis, degrees) end
---Euler angles in degrees, composed Ry * Rx * Rz (matches Transform).
---@param x number pitch
---@param y number yaw
---@param z number roll
---@return quat
function quat.from_euler(x, y, z) end

-- ---------------------------------------------------------------------------
-- Scalar helpers added to the stdlib `math` table.
-- ---------------------------------------------------------------------------

---Clamps `v` to the inclusive range [lo, hi].
---@param v number
---@param lo number
---@param hi number
---@return number
function math.clamp(v, lo, hi) end

---Linearly interpolates from `a` to `b` by `t`.
---@param a number
---@param b number
---@param t number
---@return number
function math.lerp(a, b, t) end

---Returns -1, 0, or 1 for the sign of `v`.
---@param v number
---@return number
function math.sign(v) end
