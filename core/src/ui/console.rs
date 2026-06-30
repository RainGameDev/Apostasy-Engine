use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use apostasy_macros::{Resource, start, update};
use egui::{Color32, RichText};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context as LayerContext, Layer};
use tracing_subscriber::registry::LookupSpan;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::ecs::resources::input_manager::{InputManager, KeyAction, KeyBind};
use crate::ecs::world::World;
use crate::rendering::shared::wireframe::GlobalWireframe;
use crate::ui::ui_context::EguiContext;

/// Maximum number of log lines retained by the console.
const MAX_LOGS: usize = 2000;
/// Maximum number of pending lines buffered by the global tracing capture.
const MAX_CAPTURE: usize = 2000;
/// Maximum number of history entries kept in memory and on disk.
const MAX_HISTORY: usize = 500;

const HISTORY_PATH: &str = "res/.console_history";

/// All logs found.
static LOG_CAPTURE: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());

/// Pushes a captured line into the global buffer, capping its length.
fn push_captured(entry: LogEntry) {
    if let Ok(mut buf) = LOG_CAPTURE.lock() {
        buf.push_back(entry);
        while buf.len() > MAX_CAPTURE {
            buf.pop_front();
        }
    }
}

/// The console's log capture.
/// Safe to call multiple times; only the first call takes effect.
pub fn install_log_capture() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    use tracing_subscriber::prelude::*;
    let _ = tracing_subscriber::registry()
        .with(ConsoleLogLayer)
        .try_init();

    install_stdout_capture();
}

struct ConsoleLogLayer;

impl<S> Layer<S> for ConsoleLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        // Gate out TRACE/DEBUG globally so dependencies don't even produce them.
        Some(tracing::level_filters::LevelFilter::INFO)
    }

    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let meta = event.metadata();
        // Only INFO and more severe; TRACE/DEBUG are ignored.
        if matches!(*meta.level(), Level::TRACE | Level::DEBUG) {
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            return;
        }
        push_captured(LogEntry {
            level: LogLevel::from(*meta.level()),
            text: visitor.message,
        });
    }
}

/// Returns the log level and log type of a string.
fn classify_stdout(line: &str) -> (LogLevel, String) {
    if let Some(rest) = line.strip_prefix("[ERROR!] ") {
        (LogLevel::Error, rest.to_string())
    } else if let Some(rest) = line.strip_prefix("[WARN] ") {
        (LogLevel::Warn, rest.to_string())
    } else if let Some(rest) = line.strip_prefix("[LOG] ") {
        (LogLevel::Info, rest.to_string())
    } else {
        (LogLevel::Info, line.to_string())
    }
}

/// Logs `println!("")` functions.
#[cfg(unix)]
fn install_stdout_capture() {
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::io::FromRawFd;

    unsafe {
        let mut fds = [0 as libc::c_int; 2];
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return;
        }
        let (read_fd, write_fd) = (fds[0], fds[1]);

        // Keep a handle to the real terminal so we can echo lines back to it.
        let saved_fd = libc::dup(libc::STDOUT_FILENO);
        if saved_fd < 0 {
            libc::close(read_fd);
            libc::close(write_fd);
            return;
        }

        // Point stdout at the pipe's write end.
        if libc::dup2(write_fd, libc::STDOUT_FILENO) < 0 {
            libc::close(read_fd);
            libc::close(write_fd);
            libc::close(saved_fd);
            return;
        }
        libc::close(write_fd);

        let _ = std::thread::Builder::new()
            .name("console-stdout-capture".into())
            .spawn(move || {
                let reader = BufReader::new(File::from_raw_fd(read_fd));
                let mut terminal = File::from_raw_fd(saved_fd);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    // Keep the terminal output intact.
                    let _ = writeln!(terminal, "{}", line);
                    let _ = terminal.flush();

                    let (level, text) = classify_stdout(&line);
                    if !text.is_empty() {
                        push_captured(LogEntry { level, text });
                    }
                }
            });
    }
}

#[cfg(not(unix))]
fn install_stdout_capture() {}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    /// The echo of a command the user typed.
    Command,
    /// The result returned by a command.
    Result,
}

impl From<Level> for LogLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::TRACE => LogLevel::Trace,
            Level::DEBUG => LogLevel::Debug,
            Level::INFO => LogLevel::Info,
            Level::WARN => LogLevel::Warn,
            Level::ERROR => LogLevel::Error,
        }
    }
}

impl LogLevel {
    fn color(self, visuals: &egui::Visuals) -> Color32 {
        if visuals.dark_mode {
            match self {
                LogLevel::Trace => Color32::from_rgb(100, 100, 100),
                LogLevel::Debug => Color32::from_rgb(140, 140, 140),
                LogLevel::Info => visuals.text_color(),
                LogLevel::Warn => Color32::from_rgb(220, 170, 60),
                LogLevel::Error => Color32::from_rgb(220, 80, 80),
                LogLevel::Command => Color32::from_rgb(110, 180, 240),
                LogLevel::Result => Color32::from_rgb(100, 200, 120),
            }
        } else {
            match self {
                LogLevel::Trace => Color32::from_rgb(180, 180, 180),
                LogLevel::Debug => Color32::from_rgb(130, 130, 130),
                LogLevel::Info => visuals.text_color(),
                LogLevel::Warn => Color32::from_rgb(140, 90, 0),
                LogLevel::Error => Color32::from_rgb(180, 20, 20),
                LogLevel::Command => Color32::from_rgb(20, 90, 200),
                LogLevel::Result => Color32::from_rgb(0, 120, 50),
            }
        }
    }

    fn tag(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Command => ">",
            LogLevel::Result => "=",
        }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub text: String,
}

/// The result of running a command.
pub enum CommandOutcome {
    /// Success, with a message printed as a result line.
    Success(String),
    /// Failure, with a message printed as an error line.
    Error(String),
    /// The inputs were semantically invalid; prints the command's usage.
    BadUsage,
    /// Nothing to print.
    Quiet,
}

/// A handler invoked when a command runs. Receives mutable access to the world
/// and the parsed arguments (the command name itself is not included).
pub type CommandHandler =
    Arc<dyn Fn(&mut World, &[String]) -> CommandOutcome + Send + Sync + 'static>;

#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub usage: String,
    pub description: String,
    /// Minimum number of accepted arguments.
    pub min_args: usize,
    /// Maximum number of accepted arguments, or `None` for unbounded.
    pub max_args: Option<usize>,
    pub handler: CommandHandler,
}

/// Builder for [`Command`]s. Call [`CommandBuilder::handler`] last to finish.
/// example usage:
///```rust
///#[start]
/// fn my_start(world: &mut World) -> Result<()> {
///     let mut console = world.get_resource_mut::<Console>()?;
///     console.register(
///         CommandBuilder::new("coc")
///             .usage("coc <x,z | cell_name>")
///             .description("Center on cell")
///             .args(1, Some(1))
///             .handler(|world, args| {
///                 // args[0] is the argument
///                 CommandOutcome::Success("done".into())
///             }),
///     );
///     Ok(())
/// }
/// ```
pub struct CommandBuilder {
    name: String,
    usage: Option<String>,
    description: String,
    min_args: usize,
    max_args: Option<usize>,
}

impl CommandBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            usage: None,
            description: String::new(),
            min_args: 0,
            max_args: None,
        }
    }

    /// Sets the usage string shown when the command is misused, e.g.
    /// `"spawn <model> [count]"`.
    pub fn usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = Some(usage.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Sets the accepted argument count range. `max` of `None` is unbounded.
    pub fn args(mut self, min: usize, max: Option<usize>) -> Self {
        self.min_args = min;
        self.max_args = max;
        self
    }

    /// Finishes the builder with the given handler, producing a [`Command`].
    pub fn handler(
        self,
        handler: impl Fn(&mut World, &[String]) -> CommandOutcome + Send + Sync + 'static,
    ) -> Command {
        let usage = self.usage.unwrap_or_else(|| self.name.clone());
        Command {
            name: self.name,
            usage,
            description: self.description,
            min_args: self.min_args,
            max_args: self.max_args,
            handler: Arc::new(handler),
        }
    }
}

#[derive(Resource, Clone)]
pub struct Console {
    pub open: bool,
    commands: HashMap<String, Command>,
    logs: VecDeque<LogEntry>,
    input: String,
    history: Vec<String>,
    history_cursor: Option<usize>,
    /// Set when the panel should grab keyboard focus on the next frame.
    request_focus: bool,
}

impl Default for Console {
    fn default() -> Self {
        Self {
            open: false,
            commands: HashMap::new(),
            logs: VecDeque::new(),
            input: String::new(),
            history: Vec::new(),
            history_cursor: None,
            request_focus: false,
        }
    }
}

impl Console {
    /// Loads persisted history from disk, deduplicating consecutive identical entries.
    fn load_history(&mut self) {
        let Ok(text) = std::fs::read_to_string(HISTORY_PATH) else {
            return;
        };
        for line in text.lines() {
            let line = line.trim();
            if !line.is_empty() && self.history.last().map(|l: &String| l.as_str()) != Some(line) {
                self.history.push(line.to_string());
            }
        }
        // Trim to cap in case the file grew large.
        if self.history.len() > MAX_HISTORY {
            let drop = self.history.len() - MAX_HISTORY;
            self.history.drain(..drop);
        }
    }

    /// Appends a single command to the on-disk history file.
    fn persist_history_entry(line: &str) {
        use std::io::Write;
        if let Some(parent) = std::path::Path::new(HISTORY_PATH).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(HISTORY_PATH)
        {
            let _ = writeln!(file, "{}", line);
        }
    }

    /// Registers (or replaces) a command.
    pub fn register(&mut self, command: Command) {
        self.commands.insert(command.name.clone(), command);
    }

    /// Removes a command by name.
    pub fn unregister(&mut self, name: &str) {
        self.commands.remove(name);
    }

    /// Pushes a log line at the given level.
    pub fn log(&mut self, level: LogLevel, text: impl Into<String>) {
        self.logs.push_back(LogEntry {
            level,
            text: text.into(),
        });
        while self.logs.len() > MAX_LOGS {
            self.logs.pop_front();
        }
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.log(LogLevel::Info, text);
    }

    pub fn warn(&mut self, text: impl Into<String>) {
        self.log(LogLevel::Warn, text);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.log(LogLevel::Error, text);
    }

    pub fn clear(&mut self) {
        self.logs.clear();
    }

    /// Pulls any captured tracing events into the log buffer.
    fn drain_capture(&mut self) {
        if let Ok(mut buf) = LOG_CAPTURE.lock() {
            while let Some(entry) = buf.pop_front() {
                self.logs.push_back(entry);
            }
        }
        while self.logs.len() > MAX_LOGS {
            self.logs.pop_front();
        }
    }
}

/// Toggles the `GlobalWireframe` resource.
/// Renders all meshes as wireframes.
fn toggle_wireframe(world: &mut World, _args: &[String]) -> CommandOutcome {
    if let Ok(wireframe) = world.get_resource_mut::<GlobalWireframe>() {
        wireframe.0 = !wireframe.0;
        let on = wireframe.0;
        return CommandOutcome::Success(format!("Wireframe {}", if on { "on" } else { "off" }));
    }
    world.insert_resource(GlobalWireframe(true));
    CommandOutcome::Success("Wireframe on".to_string())
}

/// Splits an input line into tokens.
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        tokens.push(current);
    }
    tokens
}

#[start(mode = "all")]
pub fn console_start(world: &mut World) -> Result<()> {
    install_log_capture();
    let mut console = Console::default();
    console.load_history();

    // Built-in commands.
    console.register(
        CommandBuilder::new("help")
            .usage("help [command]")
            .description("Lists commands, or shows usage for one command")
            .args(0, Some(1))
            .handler(|world, args| {
                let console = match world.get_resource::<Console>() {
                    Ok(c) => c,
                    Err(_) => return CommandOutcome::Error("Console unavailable".into()),
                };
                if let Some(name) = args.first() {
                    match console.commands.get(name) {
                        Some(cmd) => CommandOutcome::Success(format!(
                            "{}\n  usage: {}\n  {}",
                            cmd.name, cmd.usage, cmd.description
                        )),
                        None => CommandOutcome::Error(format!("Unknown command: '{}'", name)),
                    }
                } else {
                    let mut names: Vec<&String> = console.commands.keys().collect();
                    names.sort();
                    let mut out = String::from("Available commands:");
                    for name in names {
                        let cmd = &console.commands[name];
                        out.push_str(&format!("\n  {} - {}", cmd.name, cmd.description));
                    }
                    CommandOutcome::Success(out)
                }
            }),
    );

    console.register(
        CommandBuilder::new("clear")
            .description("Clears the console log")
            .args(0, Some(0))
            .handler(|world, _args| {
                if let Ok(console) = world.get_resource_mut::<Console>() {
                    console.clear();
                }
                CommandOutcome::Quiet
            }),
    );

    console.register(
        CommandBuilder::new("echo")
            .usage("echo <text...>")
            .description("Prints the given text back")
            .args(1, None)
            .handler(|_world, args| CommandOutcome::Success(args.join(" "))),
    );

    console.register(
        CommandBuilder::new("twf")
            .usage("twf")
            .description("Toggles wireframe rendering for everything (models, terrain, voxels)")
            .args(0, Some(0))
            .handler(toggle_wireframe),
    );

    world.insert_resource(console);

    if let Ok(inputs) = world.get_resource_mut::<InputManager>() {
        inputs.register_default_keybind(
            "Toggle Console",
            KeyBind::new(PhysicalKey::Code(KeyCode::Backquote), KeyAction::Press),
        );
    }

    Ok(())
}

#[update(mode = "all")]
pub fn console_update(world: &mut World) -> Result<()> {
    // Toggle.
    if world
        .get_resource::<InputManager>()
        .map(|i| i.is_keybind_active("Toggle Console"))
        .unwrap_or(false)
    {
        if let Ok(console) = world.get_resource_mut::<Console>() {
            console.open = !console.open;
            if console.open {
                console.request_focus = true;
            }
        }
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let mut submitted: Option<String> = None;

    {
        let console = world.get_resource_mut::<Console>()?;
        console.drain_capture();

        if console.open {
            submitted = draw_console(&ctx, console);
        }
    }

    if let Some(line) = submitted {
        execute(world, &line);
    }

    Ok(())
}

/// Draws the console window, returning a submitted command line if any.
fn draw_console(ctx: &egui::Context, console: &mut Console) -> Option<String> {
    let mut submitted = None;
    let mut open = console.open;

    egui::Window::new("Console")
        .open(&mut open)
        .resizable(true)
        .min_width(420.0)
        .min_height(120.0)
        .default_width(560.0)
        .default_height(320.0)
        .show(ctx, |ui| {
            let avail = ui.available_size_before_wrap();
            let top = ui.cursor().min;
            let full_rect = egui::Rect::from_min_size(top, avail);

            let input_block_h =
                ui.spacing().interact_size.y + ui.spacing().item_spacing.y * 2.0 + 8.0;
            let log_h = (avail.y - input_block_h).max(40.0);
            let input_h = avail.y - log_h;

            let log_rect = egui::Rect::from_min_size(top, egui::Vec2::new(avail.x, log_h));
            let input_rect = egui::Rect::from_min_size(
                top + egui::Vec2::new(0.0, log_h),
                egui::Vec2::new(avail.x, input_h),
            );

            // Log view, bounded to its rect.
            let mut log_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(log_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(&mut log_ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let visuals = ui.visuals().clone();
                    for entry in &console.logs {
                        let text = format!("[{}] {}", entry.level.tag(), entry.text);
                        ui.label(
                            RichText::new(text)
                                .monospace()
                                .size(12.0)
                                .color(entry.level.color(&visuals)),
                        );
                    }
                });

            // Command line, bounded to its rect at the bottom.
            let mut input_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(input_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            input_ui.separator();
            input_ui.horizontal(|ui| {
                ui.label(RichText::new(">").monospace().strong());
                let response = ui.add(
                    egui::TextEdit::singleline(&mut console.input)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("type a command, e.g. 'help'"),
                );

                console.input.retain(|c| c != '`');

                if console.request_focus {
                    response.request_focus();
                    console.request_focus = false;
                }

                // History navigation.
                if response.has_focus() {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        history_prev(console);
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        history_next(console);
                    }
                }

                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if enter {
                    let line = console.input.trim().to_string();
                    console.input.clear();
                    response.request_focus();
                    if !line.is_empty() {
                        Console::persist_history_entry(&line);
                        console.history.push(line.clone());
                        console.history_cursor = None;
                        submitted = Some(line);
                    }
                }
            });

            ui.advance_cursor_after_rect(full_rect);
        });

    console.open = open;
    submitted
}

fn history_prev(console: &mut Console) {
    if console.history.is_empty() {
        return;
    }
    let next = match console.history_cursor {
        Some(0) => 0,
        Some(i) => i - 1,
        None => console.history.len() - 1,
    };
    console.history_cursor = Some(next);
    console.input = console.history[next].clone();
}

fn history_next(console: &mut Console) {
    match console.history_cursor {
        Some(i) if i + 1 < console.history.len() => {
            console.history_cursor = Some(i + 1);
            console.input = console.history[i + 1].clone();
        }
        Some(_) => {
            console.history_cursor = None;
            console.input.clear();
        }
        None => {}
    }
}

/// Parses and runs a submitted command line.
fn execute(world: &mut World, line: &str) {
    if let Ok(console) = world.get_resource_mut::<Console>() {
        console.log(LogLevel::Command, line);
    }

    let tokens = tokenize(line);
    let Some(name) = tokens.first().cloned() else {
        return;
    };
    let args: Vec<String> = tokens[1..].to_vec();

    let command = world
        .get_resource::<Console>()
        .ok()
        .and_then(|c| c.commands.get(&name).cloned());

    let Some(command) = command else {
        if let Ok(console) = world.get_resource_mut::<Console>() {
            console.error(format!(
                "Unknown command: '{}'. Type 'help' for a list.",
                name
            ));
        }
        return;
    };

    // Validate argument count.
    let too_few = args.len() < command.min_args;
    let too_many = command.max_args.is_some_and(|max| args.len() > max);
    if too_few || too_many {
        log_usage(world, &command);
        return;
    }

    let outcome = (command.handler)(world, &args);
    match outcome {
        CommandOutcome::Success(msg) => {
            if let Ok(console) = world.get_resource_mut::<Console>() {
                console.log(LogLevel::Result, msg);
            }
        }
        CommandOutcome::Error(msg) => {
            if let Ok(console) = world.get_resource_mut::<Console>() {
                console.error(msg);
            }
        }
        CommandOutcome::BadUsage => log_usage(world, &command),
        CommandOutcome::Quiet => {}
    }
}

fn log_usage(world: &mut World, command: &Command) {
    if let Ok(console) = world.get_resource_mut::<Console>() {
        console.error(format!("{}: usage: {}", command.name, command.usage));
    }
}
