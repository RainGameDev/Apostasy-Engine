function start(world)
	world:log("start fired")
	local e = world:spawn()
	world:set_name(e, "LuaSpawnedThing")
	world:add_tag(e, "Player")
	world:log("spawned " .. tostring(e))
end

function update(world)
	world:log("start fired")

	local e = world:spawn()
	world:set_name(e, "LuaSpawnedThing")
	world:add_tag(e, "Player")
	world:log("spawned " .. tostring(e))
end
