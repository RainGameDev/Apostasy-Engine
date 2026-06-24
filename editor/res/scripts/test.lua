register_component("Health", { current = 100, max = 100 })
register_component("DeathTimer", { remaining = 3.0 })

function start(world)
	for i = 1, 3 do
		local e = world:spawn()
		world:set_name(e, "Enemy" .. i)
		world:add_tag(e, "Player")
		world:add_component(e, "Health", { current = i * 10, max = 100 })
	end

	world:log("--- query: Health + Player tag ---")
	world:query("Health"):with_tag("Player"):for_each(function(id, health)
		world:log(tostring(id) .. " has " .. health.current .. " hp")
		if health.current < 20 then
			world:add_component(id, "DeathTimer", { remaining = 3.0 })
		end
	end)
end

function update(world) end
