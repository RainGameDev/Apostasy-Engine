register_component("Health", { current = 100, max = 100 })
register_component("DeathTimer", { remaining = 3.0 })

---@param world World
function start(world)
	for i = 1, 3 do
		local e = world:spawn()
		world:set_name(e, "Enemy" .. i)
		world:add_tag(e, "Player")
		world:add_component(e, "Health", { current = i * 10, max = 100 })

		-- Native components: position each enemy in a row and give it velocity.
		world:add_component(e, "Transform", { local_position = { i * 2, 0, 0 } })
		world:add_component(e, "Velocity", { linear_velocity = { 0, 0, -1 } })
	end

	world:log("--- query: Health + Player tag ---")
	world:query("Health"):with_tag("Player"):for_each(function(id, health)
		world:log(tostring(id) .. " has " .. health.current .. " hp")
		if health.current < 20 then
			world:add_component(id, "DeathTimer", { remaining = 3.0 })
		end
	end)
end

---@param world World
function update(world)
	-- Native components can be queried directly now, same as script components.
	-- Nudge every Player's Transform upward over time.
	local dt = world:delta()
	world:query("Transform"):with_tag("Player"):for_each(function(id, t)
		t.local_position[2] = t.local_position[2] + dt
		world:set_component(id, "Transform", { local_position = t.local_position })
	end)
end

---@param world World
---@param delta number
function fixed_update(world, delta)
	world:query("DeathTimer"):for_each(function(id, timer)
		timer.remaining = timer.remaining - delta
		if timer.remaining <= 0 then
			world:despawn(id)
		else
			world:set_component(id, "DeathTimer", timer)
		end
	end)
end

---@param world World
function late_update(world) end

---@param world World
function prerender(world) end
