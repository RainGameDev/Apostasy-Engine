-- Player stats, seeded once and persisted across frames as a Lua resource.
register_resource("player", {
	health = 87,
	max_health = 100,
	stamina = 63,
	max_stamina = 100,
	gold = 42,
})
---@param world World
function start(world)
	world:load_worldspace("default")

	-- camera
	local camera = world:spawn()
	world:add_component(
		camera,
		"Transform",
		{ local_position = { 0.0, 1.6, 0.0 }, local_euler_angles = { 0.0, 180.0, 0.0 } }
	)
	world:add_component(camera, "Camera", { fov_y = 90.0, is_main = true })
	world:add_tag(camera, "ActiveCamera")
	world:set_parent(camera, player)
end

-- Rotates a 3-vector `v` (sequence {x,y,z}) by quaternion `q` (sequence {x,y,z,w}).
local function rotate_vector_by_quaternion(q, v)
	local qx, qy, qz, qw = q[1], q[2], q[3], q[4]
	local vx, vy, vz = v[1], v[2], v[3]

	local tx = 2 * (qy * vz - qz * vy)
	local ty = 2 * (qz * vx - qx * vz)
	local tz = 2 * (qx * vy - qy * vx)

	return {
		vx + qw * tx + (qy * tz - qz * ty),
		vy + qw * ty + (qz * tx - qx * tz),
		vz + qw * tz + (qx * ty - qy * tx),
	}
end

---@param world World
function update(world)
	local move = world:input_vector_2d("Left", "Right", "Forwards", "Backwards")
	local should_jump = world:is_keybind_active("Jump")
	local delta = world:mouse_delta()

	world:query("Transform"):with_tag("Player"):for_each(function(id, transform)
		transform.local_euler_angles[2] = transform.local_euler_angles[2] - delta.x
		world:set_component(id, "Transform", transform)
	end)

	world:query("Transform"):with_tag("ActiveCamera"):for_each(function(id, transform)
		local pitch = transform.local_euler_angles[1] - delta.y
		transform.local_euler_angles[1] = math.max(-89, math.min(89, pitch))
		world:set_component(id, "Transform", transform)
	end)

	-- velocity: move relative to the player's own yaw (not the camera's pitch)
	local MOVE_SPEED = 6.0
	local len = math.sqrt(move.x * move.x + move.y * move.y)
	local nx, ny = move.x, move.y
	if len > 1.0 then
		nx, ny = nx / len, ny / len
	end

	world:query("Transform", "Velocity"):with_tag("Player"):for_each(function(id, transform, velocity)
		local wish = rotate_vector_by_quaternion(transform.global_rotation, { nx, 0.0, -ny })

		velocity.linear_velocity[1] = wish[1] * MOVE_SPEED
		velocity.linear_velocity[3] = wish[3] * MOVE_SPEED

		if should_jump and velocity.is_grounded then
			velocity.linear_velocity[2] = 15.0
		end

		world:set_component(id, "Velocity", velocity)
	end)
end
