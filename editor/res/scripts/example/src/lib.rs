use apostasy_script_api::*;

/// Spawns a camera and a wooden crate when the engine starts.
#[unsafe(no_mangle)]
pub extern "C" fn on_start() {
    spawn("Camera")
        .with(Camera::default())
        .with(Transform {
            local_position: Vector3::new(0.0, 2.0, 20.0),
            local_scale: Vector3::one(),
            ..Default::default()
        })
        .add_tag::<ActiveCamera>()
        .add_tag::<GameCamera>();

    spawn("Crate")
        .with(Transform::at(0.0, 0.5, 0.0))
        .with(ModelRenderer::from_path("cube"))
        .with(Collider::cuboid(0.5, 0.5, 0.5));

    log("example script: on_start complete.");
}

/// Slowly rotates the crate each frame.
#[unsafe(no_mangle)]
pub extern "C" fn on_update() {
    log(&format!("time: {:.2}", time()));
}
