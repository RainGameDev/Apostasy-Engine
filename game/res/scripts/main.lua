-- Player stats, seeded once and persisted across frames as a Lua resource.
register_resource("player", {
	health = 87,
	max_health = 100,
	stamina = 63,
	max_stamina = 100,
	gold = 42,
})

-- Rolling FPS tracker (updated every 30 frames).
local fps_acc = 0.0
local fps_count = 0
local fps_display = 60.0

---@param world World
function start(world)
	world:load_worldspace("default")

	-- Camera — "FlyCamera" is the Rust tag in game/src/main.rs that the
	-- movement + mouse-look system looks for.
	local cam = world:spawn()
	world:set_name(cam, "Camera")
	world:add_component(cam, "Transform", {
		local_position = { 0, 4, 12 },
		local_euler_angles = { -10, 0, 0 },
	})
	world:add_component(cam, "Camera", { fov_y = 90.0, is_main = true })
	world:add_tag(cam, "ActiveCamera")
	world:add_tag(cam, "FlyCamera")

	world:log("[game] scene ready")
end

---@param world World
function update(world)
	local dt = world:delta()

	-- Rolling 30-frame FPS average.
	fps_acc = fps_acc + dt
	fps_count = fps_count + 1
	if fps_count >= 30 then
		fps_display = fps_count / fps_acc
		fps_acc = 0.0
		fps_count = 0
	end

	-- Slowly rotate the gem every frame.
	if gem_entity then
		local t = world:get_component(gem_entity, "Transform")
		if t then
			local e = t.local_euler_angles
			world:set_component(gem_entity, "Transform", {
				local_euler_angles = { e[1], e[2] + 45 * dt, e[3] },
			})
		end
	end

	local player = world:get_resource("player")
	if not player then
		return
	end

	-- -- HUD — pinned top-left, no title bar ----------------------------------

	world:ui_window(
		"HUD",
		{ anchor = "top_left", offset = { 10, 10 }, no_title_bar = true, resizable = false },
		function(ui)
			-- FPS (green when fast, amber when dropping).
			local fps_col = fps_display >= 50 and { r = 120, g = 220, b = 120, a = 255 }
				or { r = 240, g = 180, b = 60, a = 255 }
			ui:colored_label(fps_col, string.format("FPS  %.0f", fps_display))
			ui:colored_label({ r = 140, g = 140, b = 140, a = 200 }, string.format("Time  %.1f s", world:time()))

			ui:separator()
		end
	)
end
