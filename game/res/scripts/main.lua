-- Player stats, seeded once and persisted across frames as a Lua resource.
register_resource("player", {
	health = 87,
	max_health = 100,
	stamina = 63,
	max_stamina = 100,
	gold = 42,
})

-- Entities captured during start() and reused in update().
local gem_entity = nil

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

	local dirty = false -- set to true if we modified player this frame

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

			-- Health bar.
			local hp_frac = player.health / player.max_health
			local hp_col = hp_frac > 0.4 and { r = 80, g = 200, b = 80, a = 255 }
				or { r = 220, g = 60, b = 60, a = 255 }
			ui:colored_label(hp_col, string.format("HP  %d / %d", player.health, player.max_health))
			ui:progress_bar(hp_frac)

			-- Stamina bar.
			ui:colored_label(
				{ r = 80, g = 160, b = 240, a = 255 },
				string.format("SP  %d / %d", player.stamina, player.max_stamina)
			)
			ui:progress_bar(player.stamina / player.max_stamina)

			ui:separator()
			ui:colored_label({ r = 240, g = 200, b = 60, a = 255 }, string.format("Gold  %d", player.gold))
		end
	)

	-- -- Debug panel -----------------------------------------------------------

	world:ui_window("Debug", function(ui)
		ui:heading("Player Stats")

		local new_hp = ui:slider("Health", player.health, 0, player.max_health)
		if new_hp ~= player.health then
			player.health = math.floor(new_hp + 0.5)
			dirty = true
		end

		local new_sp = ui:slider("Stamina", player.stamina, 0, player.max_stamina)
		if new_sp ~= player.stamina then
			player.stamina = math.floor(new_sp + 0.5)
			dirty = true
		end

		local new_gold = ui:drag("Gold", player.gold, 1.0)
		if new_gold ~= player.gold then
			player.gold = math.floor(new_gold + 0.5)
			dirty = true
		end

		ui:separator()

		-- Buttons side by side.
		ui:horizontal(function(row)
			if row:button("Full heal") then
				player.health = player.max_health
				player.stamina = player.max_stamina
				dirty = true
			end
			if row:button("Damage -15") then
				player.health = math.max(0, player.health - 15)
				dirty = true
			end
		end)

		ui:separator()

		-- Collapsible extras section.
		ui:collapsing("Extras", function(inner)
			-- Name editor.
			player.name = player.name or "Hero"
			local new_name = inner:text_input("Name", player.name)
			if new_name ~= player.name then
				player.name = new_name
				dirty = true
			end

			inner:space(4)

			-- Two-column stat display.
			inner:columns(2, function(cols)
				cols[1]:small("Max HP")
				cols[1]:label(tostring(player.max_health))
				cols[2]:small("Max SP")
				cols[2]:label(tostring(player.max_stamina))
			end)
		end)
	end)

	-- -- Controls hint — pinned bottom-right -----------------------------------

	world:ui_window(
		"Controls",
		{ anchor = "bottom_right", offset = { -10, -10 }, no_title_bar = true, resizable = false },
		function(ui)
			ui:small("WASD · Shift  move / sprint")
			ui:small("Mouse         look")
			ui:small("Esc           pause")
		end
	)

	if dirty then
		world:set_resource("player", player)
	end
end
