---@diagnostic disable: duplicate-set-field, lowercase-global
-- Apostasy Lua prelude: math ergonomics.
--
-- Defines `vec2`, `vec3`, `vec4`, and `quat` as global constructors. These are
-- plain Lua tables whose array part holds the components (`{x, y, z}`) — all type
-- information lives in the metatable, so an instance has NO string keys of its own.
-- That matters: it lets a vector serialize straight to a `[x, y, z]` yaml sequence
-- when passed to `set_component`, exactly matching how the engine stores vectors.
--
--   local p = vec3(world:get_component(id, "Transform").local_position)
--   p = p + vec3(0, world:delta(), 0)
--   world:set_component(id, "Transform", { local_position = p })

local COMP = { x = 1, y = 2, z = 3, w = 4 }

-- Builds a vector type of `dim` components named `name` (e.g. "vec3").
-- Returns a callable constructor table.
local function make_vec(dim, name)
	local methods = {}
	local mt = {}

	-- Wraps an existing array table in this type's metatable (no copy).
	local function wrap(t)
		return setmetatable(t, mt)
	end

	-- Constructs from: nothing (zero), a single number (splat), a table
	-- (array or {x=,y=,z=}), or explicit components. Extra args past `dim`
	-- are ignored; missing ones default to 0.
	local function new(a, b, c, d)
		local src
		if type(a) == "table" then
			src = { a.x or a[1] or 0, a.y or a[2] or 0, a.z or a[3] or 0, a.w or a[4] or 0 }
		elseif type(a) == "number" and b == nil then
			src = { a, a, a, a }
		else
			src = { a or 0, b or 0, c or 0, d or 0 }
		end
		local t = {}
		for i = 1, dim do
			t[i] = src[i] or 0
		end
		return wrap(t)
	end

	-- Field access: `.x`/`.y`/`.z`/`.w` map to array slots; otherwise a method.
	mt.__index = function(v, k)
		local i = COMP[k]
		if i and i <= dim then
			return rawget(v, i)
		end
		return methods[k]
	end

	-- Only the named components may be assigned, keeping instances key-clean so
	-- they still serialize as sequences. `v.x = 5` works; `v.foo = 5` errors.
	mt.__newindex = function(v, k, val)
		local i = COMP[k]
		if i and i <= dim then
			rawset(v, i, val)
		else
			error(name .. " has no field '" .. tostring(k) .. "'", 2)
		end
	end

	mt.__add = function(a, b)
		local t = {}
		for i = 1, dim do
			t[i] = rawget(a, i) + rawget(b, i)
		end
		return wrap(t)
	end

	mt.__sub = function(a, b)
		local t = {}
		for i = 1, dim do
			t[i] = rawget(a, i) - rawget(b, i)
		end
		return wrap(t)
	end

	-- `*` is scalar when either side is a number, component-wise between vectors.
	mt.__mul = function(a, b)
		if type(a) == "number" then
			a, b = b, a
		end
		local t = {}
		if type(b) == "number" then
			for i = 1, dim do
				t[i] = rawget(a, i) * b
			end
		else
			for i = 1, dim do
				t[i] = rawget(a, i) * rawget(b, i)
			end
		end
		return wrap(t)
	end

	mt.__div = function(a, b)
		local t = {}
		if type(b) == "number" then
			for i = 1, dim do
				t[i] = rawget(a, i) / b
			end
		else
			for i = 1, dim do
				t[i] = rawget(a, i) / rawget(b, i)
			end
		end
		return wrap(t)
	end

	mt.__unm = function(a)
		local t = {}
		for i = 1, dim do
			t[i] = -rawget(a, i)
		end
		return wrap(t)
	end

	mt.__eq = function(a, b)
		for i = 1, dim do
			if rawget(a, i) ~= rawget(b, i) then
				return false
			end
		end
		return true
	end

	mt.__len = function()
		return dim
	end

	mt.__tostring = function(v)
		local parts = {}
		for i = 1, dim do
			parts[i] = tostring(rawget(v, i))
		end
		return name .. "(" .. table.concat(parts, ", ") .. ")"
	end

	function methods:length_squared()
		local s = 0
		for i = 1, dim do
			s = s + rawget(self, i) * rawget(self, i)
		end
		return s
	end

	function methods:length()
		return math.sqrt(self:length_squared())
	end

	function methods:normalized()
		local len = self:length()
		if len == 0 then
			return new()
		end
		return self / len
	end

	function methods:dot(other)
		local s = 0
		for i = 1, dim do
			s = s + rawget(self, i) * rawget(other, i)
		end
		return s
	end

	function methods:distance(other)
		return (self - other):length()
	end

	-- Linear interpolation toward `other` by `t` (0..1).
	function methods:lerp(other, t)
		return self + (other - self) * t
	end

	function methods:copy()
		return new(self)
	end

	-- Returns the components as plain multiple values: `local x, y, z = v:unpack()`.
	function methods:unpack()
		return table.unpack(self, 1, dim)
	end

	if dim == 3 then
		function methods:cross(o)
			return new(
				rawget(self, 2) * rawget(o, 3) - rawget(self, 3) * rawget(o, 2),
				rawget(self, 3) * rawget(o, 1) - rawget(self, 1) * rawget(o, 3),
				rawget(self, 1) * rawget(o, 2) - rawget(self, 2) * rawget(o, 1)
			)
		end
	end

	local ctor = setmetatable({}, {
		__call = function(_, a, b, c, d)
			return new(a, b, c, d)
		end,
	})
	ctor.new = new
	return ctor, new
end

local vec2 = make_vec(2, "vec2")
local vec3, new_vec3 = make_vec(3, "vec3")
local vec4 = make_vec(4, "vec4")

-- Handy vec3 constants (engine forward is -Z, matching Transform).
vec3.zero = vec3(0, 0, 0)
vec3.one = vec3(1, 1, 1)
vec3.up = vec3(0, 1, 0)
vec3.down = vec3(0, -1, 0)
vec3.right = vec3(1, 0, 0)
vec3.left = vec3(-1, 0, 0)
vec3.forward = vec3(0, 0, -1)
vec3.back = vec3(0, 0, 1)

-- ---------------------------------------------------------------------------
-- quat: a unit quaternion (x, y, z, w). Stored as `{x, y, z, w}` like vec4, but
-- `*` performs quaternion multiplication (composition of rotations).
-- ---------------------------------------------------------------------------
local quat_methods = {}
local quat_mt = {}

local function wrap_quat(t)
	return setmetatable(t, quat_mt)
end

local function new_quat(x, y, z, w)
	if type(x) == "table" then
		return wrap_quat({ x.x or x[1] or 0, x.y or x[2] or 0, x.z or x[3] or 0, x.w or x[4] or 1 })
	end
	return wrap_quat({ x or 0, y or 0, z or 0, w or 1 })
end

quat_mt.__index = function(q, k)
	local i = COMP[k]
	if i then
		return rawget(q, i)
	end
	return quat_methods[k]
end

quat_mt.__newindex = function(q, k, val)
	local i = COMP[k]
	if i then
		rawset(q, i, val)
	else
		error("quat has no field '" .. tostring(k) .. "'", 2)
	end
end

-- Hamilton product: composes two rotations (apply `b` then `a`).
quat_mt.__mul = function(a, b)
	local ax, ay, az, aw = a.x, a.y, a.z, a.w
	local bx, by, bz, bw = b.x, b.y, b.z, b.w
	return wrap_quat({
		aw * bx + ax * bw + ay * bz - az * by,
		aw * by - ax * bz + ay * bw + az * bx,
		aw * bz + ax * by - ay * bx + az * bw,
		aw * bw - ax * bx - ay * by - az * bz,
	})
end

quat_mt.__eq = function(a, b)
	return a.x == b.x and a.y == b.y and a.z == b.z and a.w == b.w
end

quat_mt.__tostring = function(q)
	return string.format("quat(%s, %s, %s, %s)", q.x, q.y, q.z, q.w)
end

function quat_methods:length()
	return math.sqrt(self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w)
end

function quat_methods:normalized()
	local len = self:length()
	if len == 0 then
		return new_quat(0, 0, 0, 1)
	end
	return new_quat(self.x / len, self.y / len, self.z / len, self.w / len)
end

function quat_methods:conjugate()
	return new_quat(-self.x, -self.y, -self.z, self.w)
end

-- For a unit quaternion the conjugate is the inverse.
function quat_methods:inverse()
	return self:conjugate()
end

-- Rotates a vec3 by this quaternion. Uses the standard
-- `v + 2w(u x v) + 2(u x (u x v))` formulation.
function quat_methods:rotate(v)
	local ux, uy, uz, w = self.x, self.y, self.z, self.w
	local tx = 2 * (uy * v.z - uz * v.y)
	local ty = 2 * (uz * v.x - ux * v.z)
	local tz = 2 * (ux * v.y - uy * v.x)
	return new_vec3(
		v.x + w * tx + (uy * tz - uz * ty),
		v.y + w * ty + (uz * tx - ux * tz),
		v.z + w * tz + (ux * ty - uy * tx)
	)
end

local quat = setmetatable({}, {
	__call = function(_, x, y, z, w)
		return new_quat(x, y, z, w)
	end,
})

quat.identity = function()
	return new_quat(0, 0, 0, 1)
end

-- Rotation of `degrees` about a (normalized) axis vec3.
quat.from_axis_angle = function(axis, degrees)
	local a = vec3(axis):normalized()
	local half = math.rad(degrees) * 0.5
	local s = math.sin(half)
	return new_quat(a.x * s, a.y * s, a.z * s, math.cos(half))
end

-- Euler angles in degrees, composed Ry * Rx * Rz to match the engine's
-- Transform.local_euler_angles convention.
quat.from_euler = function(x, y, z)
	local qx = quat.from_axis_angle(vec3.right, x)
	local qy = quat.from_axis_angle(vec3.up, y)
	local qz = quat.from_axis_angle(vec3(0, 0, 1), z)
	return qy * qx * qz
end

-- ---------------------------------------------------------------------------
-- Scalar math helpers (added to the stdlib `math` table).
-- ---------------------------------------------------------------------------
function math.clamp(v, lo, hi)
	if v < lo then
		return lo
	elseif v > hi then
		return hi
	end
	return v
end

function math.lerp(a, b, t)
	return a + (b - a) * t
end

function math.sign(v)
	if v > 0 then
		return 1
	elseif v < 0 then
		return -1
	end
	return 0
end

-- Publish to globals so sandboxed scripts inherit them.
_G.vec2 = vec2
_G.vec3 = vec3
_G.vec4 = vec4
_G.quat = quat
