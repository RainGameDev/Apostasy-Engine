function start(world)
	world:register_component("Health", { current = 100, max = 100 })

	local e = world:spawn()
	world:set_name(e, "Hero")
	world:add_component(e, "Health")
	world:add_component(e, "Mana", { current = 50 })

	local h = world:get_component(e, "Health")
	world:log("Health: " .. h.current .. "/" .. h.max)

	h.current = h.current - 30
	world:set_component(e, "Health", h)
	world:log("After damage: " .. world:get_component(e, "Health").current)

	world:log("has Mana? " .. tostring(world:has_component(e, "Mana")))
	world:remove_component(e, "Mana")
	world:log("has Mana? " .. tostring(world:has_component(e, "Mana")))
end

function update(world) end
