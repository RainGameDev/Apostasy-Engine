register_component("Orbit", { radius = 5.0, speed = 30.0, angle = 0.0, axis = { 0, 1, 0 } })
-- `probed` is a one-shot latch for the raycast demo below.
register_resource("SimConfig", { paused = false, time_scale = 1.0, probed = false })

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
	-- A sphere collider so the body can be hit by raycasts.
	world:add_component(e, "Collider", { shape = "Sphere", radius = 1.0 })
	-- A visible mesh (one of the engine's built-in default models).
	world:add_component(e, "ModelRenderer", { model_path = "m_default_sphere" })
	world:set_parent(e, parent)
	return e
end

---Finds the first entity with the given display name, or nil.
---@param world World
---@param name string
---@return Entity|nil
local function find_by_name(world, name)
	for _, id in ipairs(world:get_all_entities()) do
		if world:get_name(id) == name then
			return id
		end
	end
	return nil
end

---@param world World
function start(world)
	-- A camera looking down at the system, so the scene is visible in-game.
	-- "ActiveCamera" is a real engine tag; the renderer uses it to pick the camera.
	local camera = world:spawn()
	world:set_name(camera, "Camera")
	world:add_component(camera, "Transform", { local_position = { 0, 18, 32 }, local_euler_angles = { -29, 0, 0 } })
	world:add_component(camera, "Camera", { fov_y = 70.0 })
	world:add_tag(camera, "ActiveCamera")

	-- A directional light so every body is lit regardless of where it orbits.
	local sun = world:spawn()
	world:set_name(sun, "Sunlight")
	world:add_component(sun, "Transform", { local_euler_angles = { -50, -30, 0 } })
	world:add_component(sun, "Light", {
		light_type = "Directional",
		color = { r = 1.0, g = 1.0, b = 1.0 },
		intensity = 2.0,
		is_emitting = true,
	})

	-- The star sits at the origin: a bright sphere that also emits a point light.
	local star = world:spawn()
	world:set_name(star, "Star")
	world:add_component(star, "Transform", { local_position = { 0, 0, 0 }, local_scale = { 2, 2, 2 } })
	world:add_component(star, "ModelRenderer", { model_path = "m_default_sphere" })
	world:add_component(star, "Light", {
		light_type = "Point",
		radius = 50.0,
		color = { r = 1.0, g = 0.9, b = 0.6 },
		intensity = 5.0,
		is_emitting = true,
	})

	-- Three planets orbiting the star, plus a moon orbiting the middle planet.
	spawn_body(world, star, "Planet-A", 6.0, 40.0)
	local planet_b = spawn_body(world, star, "Planet-B", 10.0, 25.0)
	spawn_body(world, star, "Planet-C", 14.0, 15.0)
	local moon = spawn_body(world, planet_b, "Moon", 2.5, 120.0)

	-- A free-floating beacon, spawned directly at a world position rather than
	-- being parented and offset.
	local beacon = world:spawn_at_position(vec3(0, 8, 0))
	world:set_name(beacon, "Beacon")

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

	-- Raycast demo: once the bodies have moved (after the first frame, so global
	-- transforms are settled), cast a ray from the star toward Planet-A and
	-- report what the ray strikes. Latched so it only fires once.
	if cfg and not cfg.probed then
		local star = find_by_name(world, "Star")
		local target = find_by_name(world, "Planet-A")
		if star and target then
			-- Planet-A is parented to the star at the origin, so its local
			-- position equals its world position — fine to aim with.
			local dir = vec3(world:get_component(target, "Transform").local_position)
			local hit = world:raycast(vec3.zero, dir, 100.0, star)
			if hit then
				world:log(string.format(
					"raycast hit '%s' at distance %.2f, normal %s",
					world:get_name(hit.entity) or tostring(hit.entity),
					hit.distance,
					tostring(vec3(hit.normal))
				))
			else
				world:log("raycast hit nothing")
			end
			cfg.probed = true
			world:set_resource("SimConfig", cfg)
		end
	end
end

---@param world World
---@param delta number
function fixed_update(world, delta) end

---@param world World
function late_update(world) end

---@param world World
function prerender(world) end
