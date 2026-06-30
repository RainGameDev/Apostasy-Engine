-- A script-defined component the `update` loop below spins each frame.
register_component("Spin", { speed_x = 25.0, speed_y = 60.0 })

---@param world World
function start(world)
	-- Fly camera. "ActiveCamera" is what the renderer draws from; "FlyCamera"
	-- is the Rust tag the mouse-look/movement system in main.rs looks for.
	local cam = world:spawn()
	world:set_name(cam, "Player Camera")
	world:add_component(cam, "Transform", {
		local_position = { 0, 4, 12 },
		local_euler_angles = { -10, 0, 0 },
	})
	world:add_component(cam, "Camera", { fov_y = 90.0, is_main = true })
	world:add_tag(cam, "ActiveCamera")
	world:add_tag(cam, "FlyCamera")

	-- Directional "sun".
	local sun = world:spawn()
	world:set_name(sun, "Sun")
	world:add_component(sun, "Transform", { local_euler_angles = { -50, -30, 0 } })
	world:add_component(sun, "Light", {
		light_type = "Directional",
		color = { r = 1.0, g = 0.96, b = 0.85 },
		intensity = 1.6,
		is_emitting = true,
	})

	-- Ground plane.
	local ground = world:spawn()
	world:set_name(ground, "Ground")
	world:add_component(ground, "Transform", { local_scale = { 40, 1, 40 } })
	world:add_component(ground, "ModelRenderer", { model_path = "m_default_plane" })

	-- A ring of alternating cubes and spheres around the origin.
	local count = 8
	for i = 0, count - 1 do
		local angle = i / count * (2 * math.pi)
		local radius = 6.0
		local prop = world:spawn()
		world:set_name(prop, "Prop " .. i)
		world:add_component(prop, "Transform", {
			local_position = { math.cos(angle) * radius, 0.6, math.sin(angle) * radius },
			local_scale = { 0.6, 0.6, 0.6 },
		})
		local model = (i % 2 == 0) and "m_default_cube" or "m_default_sphere"
		world:add_component(prop, "ModelRenderer", { model_path = model })
	end

	-- A spinning cube floating in the centre, driven by the Lua `update` below.
	local spinner = world:spawn()
	world:set_name(spinner, "Spinner")
	world:add_component(spinner, "Transform", { local_position = { 0, 2.5, 0 } })
	world:add_component(spinner, "ModelRenderer", { model_path = "m_default_cube" })
	world:add_component(spinner, "Spin", {})

	-- Campfire model with a warm, flickering point light.
	local fire = world:spawn()
	world:set_name(fire, "Campfire")
	world:add_component(fire, "Transform", {})
	world:add_component(fire, "ModelRenderer", { model_path = "M_Campfire" })

	local fire_light = world:spawn()
	world:set_name(fire_light, "Campfire Light")
	world:add_component(fire_light, "Transform", { local_position = { 0, 1, 0 } })
	world:add_component(fire_light, "Light", {
		light_type = "Point",
		radius = 14.0,
		color = { r = 1.0, g = 0.55, b = 0.2 },
		intensity = 3.0,
		is_emitting = true,
		is_flickering = true,
		intensity_min = 2.0,
		intensity_max = 3.5,
	})

	world:log("[demo] scene ready")
end

---@param world World
function update(world)
	local dt = world:delta()
	world:query("Spin"):for_each(function(id, spin)
		local t = world:get_component(id, "Transform")
		local e = t.local_euler_angles
		world:set_component(id, "Transform", {
			local_euler_angles = {
				e[1] + spin.speed_x * dt,
				e[2] + spin.speed_y * dt,
				e[3],
			},
		})
	end)
end
