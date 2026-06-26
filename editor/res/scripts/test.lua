register_component("Orbit", { radius = 5.0, speed = 30.0, angle = 0.0, axis = { 0, 1, 0 } })
register_resource("SimConfig", { paused = false, time_scale = 1.0 })
---@param world World
---@param parent Entity
---@param name string
---@param radius number
---@param speed number
local function spawn_body(world, parent, name, radius, speed)
	local e = world:spawn()
	world:set_name(e, name)
	world:add_component(e, "Transform", { local_position = { radius, 0, 0 } })
	world:add_component(e, "Orbit", { radius = radius, speed = speed, axis = { 0, 1, 0 } })
	world:set_parent(e, parent)
	return e
end

---@param world World
function start(world)
	-- The star sits at the origin and emits light.
	local star = world:spawn()
	world:set_name(star, "Star")
	world:add_component(star, "Transform", { local_position = { 0, 0, 0 } })
	world:add_component(star, "Light", {
		light_type = "Point",
		radius = 50.0,
		color = { r = 1.0, g = 0.9, b = 0.6 },
		intensity = 5.0,
		is_emitting = true,
	})
	world:add_tag(star, "Star")

	-- Three planets orbiting the star, plus a moon orbiting the middle planet.
	spawn_body(world, star, "Planet-A", 6.0, 40.0)
	local planet_b = spawn_body(world, star, "Planet-B", 10.0, 25.0)
	spawn_body(world, star, "Planet-C", 14.0, 15.0)
	local moon = spawn_body(world, planet_b, "Moon", 2.5, 120.0)

	-- Hierarchy readout: the star's direct children, then the moon's ancestry.
	world:log("--- Star system ---")
	for _, child in ipairs(world:get_children(star)) do
		world:log("orbiting star: " .. (world:get_name(child) or tostring(child)))
	end
	for _, ancestor in ipairs(world:get_ancestors(moon)) do
		world:log("moon ancestor: " .. (world:get_name(ancestor) or tostring(ancestor)))
	end

	-- Native-component query: count the lights in the scene.
	local lights = 0
	world:query("Light"):for_each(function()
		lights = lights + 1
	end)
	world:log("light count: " .. lights)
end

---@param world World
function update(world)
	local cfg = world:get_resource("SimConfig")
	if cfg and cfg.paused then
		return
	end
	local dt = world:delta() * (cfg and cfg.time_scale or 1.0)

	world:query("Orbit"):for_each(function(id, orbit)
		orbit.angle = (orbit.angle + orbit.speed * dt) % 360.0
		local rotation = quat.from_axis_angle(orbit.axis, orbit.angle)
		local position = rotation:rotate(vec3(orbit.radius, 0, 0))
		world:set_component(id, "Transform", { local_position = position })
		world:set_component(id, "Orbit", orbit) -- persist the advanced angle
	end)
end

---@param world World
---@param delta number
function fixed_update(world, delta) end

---@param world World
function late_update(world) end

---@param world World
function prerender(world) end
