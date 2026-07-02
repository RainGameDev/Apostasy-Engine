//! Compiles every .hail script in the repo against the real engine module,
//! so script/API drift fails here instead of at game startup.

use std::path::PathBuf;
use std::sync::Arc;

use apostasy_core::scripting::hail::resolver::FileImportResolver;
use apostasy_core::scripting::hail::runtime::build_engine_module;

fn compile(source: &str, path: &str) -> Result<hail::Program, String> {
    let module = Arc::new(build_engine_module().map_err(|e| format!("module: {e:?}"))?);
    let resolver = Arc::new(FileImportResolver::new());

    let mut scanner = hail::Scanner::new(source.to_string());
    scanner.scan();
    let tokens = scanner.get().ok_or("no tokens")?.to_vec();
    if !scanner.errors().is_empty() {
        return Err(hail::format_diagnostics(
            source,
            Some(path),
            scanner.errors(),
            &tokens,
        ));
    }

    let mut parser = hail::Parser::new(&tokens);
    parser.parse();
    if !parser.errors().is_empty() {
        return Err(hail::format_diagnostics(
            source,
            Some(path),
            parser.errors(),
            &tokens,
        ));
    }
    let stmts = parser.get().ok_or("no statements")?;

    let construct = hail::Constructor::new(module.clone(), resolver.clone(), Some(path.to_string()))
        .generate(stmts)
        .map_err(|e| hail::format_diagnostic(source, Some(path), &e, &tokens))?;

    Ok(hail::Instructor::new(module, resolver, None).generate(
        construct,
        source.to_string(),
        tokens,
    ))
}

/// The hail port of the Lua player controller: queries with tag filters,
/// transform/velocity snapshots, input, and rotation all in one script.
#[test]
fn player_controller_port_compiles() {
    let source = r#"
const MOVE_SPEED = 6.0;

fn update(world: World) {
    let move_input = world.input_vector_2d("Left", "Right", "Forwards", "Backwards");
    let should_jump = world.is_keybind_active("Jump");
    let delta = world.mouse_delta();

    for e in world.query.with("Transform").with_tag("Player").run() {
        let mut t = world.get_transform(e).unwrap();
        t.euler_angles = vec3(t.euler_angles.x, t.euler_angles.y - delta.x, t.euler_angles.z);
        world.set_transform(e, t);
    }

    for e in world.query.with("Transform").with_tag("ActiveCamera").run() {
        let mut t = world.get_transform(e).unwrap();
        let pitch = (t.euler_angles.x - delta.y).max(0.0 - 89.0).min(89.0);
        t.euler_angles = vec3(pitch, t.euler_angles.y, t.euler_angles.z);
        world.set_transform(e, t);
    }

    let len = (move_input.x * move_input.x + move_input.y * move_input.y).sqrt();
    let mut nx = move_input.x;
    let mut ny = move_input.y;
    if len > 1.0 {
        nx = nx / len;
        ny = ny / len;
    }

    for e in world.query.with("Transform").with("Velocity").with_tag("Player").run() {
        let t = world.get_transform(e).unwrap();
        let wish = t.rotate(vec3(nx, 0.0, 0.0 - ny));

        let mut v = world.get_velocity(e).unwrap();
        v.linear_velocity = vec3(wish.x * MOVE_SPEED, v.linear_velocity.y, wish.z * MOVE_SPEED);

        if should_jump {
            if v.is_grounded {
                v.linear_velocity = vec3(v.linear_velocity.x, 15.0, v.linear_velocity.z);
            }
        }

        world.set_velocity(e, v);
    }
}
"#;
    if let Err(diag) = compile(source, "player_controller_port.hail") {
        panic!("{diag}");
    }
}

#[test]
fn game_scripts_compile() {
    let scripts_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../game/res/scripts");
    let mut checked = 0;
    for entry in walkdir::WalkDir::new(&scripts_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "hail") {
            let source = std::fs::read_to_string(path).expect("script should be readable");
            if let Err(diag) = compile(&source, &path.to_string_lossy()) {
                panic!("{diag}");
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "no .hail scripts found in {scripts_dir:?}");
}
