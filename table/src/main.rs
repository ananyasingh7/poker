// ABOUTME: The poker-table program: reads its options, starts the game thread and shows the
// ABOUTME: pixel canvas in a window at a whole-number scale (or writes pictures with --snapshot).
use poker_table::{
    app::{App, Input, Key, Tab, View},
    canvas::{Canvas, H, W},
    mock,
    session::{self, Brain, Env, Phase, Settings, Shared},
    theme,
};
use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key as WinitKey, NamedKey},
    window::{Window, WindowId},
};

const USAGE: &str = "poker-table: watch a model play six-handed poker against the lab's bots

USAGE (from the poker-lab directory, so the key file and runs/ are found):
  poker-table                 open the table; choose the player and help, then start
  poker-table --mock          the model is a local mock: free, offline, canned reasoning
  poker-table --open DIR      replay and analyse a saved session (a directory with evaluator.jsonl)
  poker-table --runs DIR      where to save sessions (default: runs)
  poker-table --no-save       do not save the session
  poker-table --snapshot DIR  write pictures of every screen (mock model) and exit

KEYS  space pause/resume   c settings   h show all cards   tab or 1-5 panel tabs
      left/right step through hands   up/down previous/next hand   l or esc back to live   +/- speed";

struct Options {
    mock: bool,
    open: Option<PathBuf>,
    runs: Option<PathBuf>,
    snapshot: Option<PathBuf>,
}

fn options() -> Result<Options, String> {
    let mut options = Options {
        mock: false,
        open: None,
        runs: Some(PathBuf::from("runs")),
        snapshot: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut path = |name: &str| {
            args.next()
                .map(PathBuf::from)
                .ok_or(format!("{name} needs a directory"))
        };
        match arg.as_str() {
            "--mock" => options.mock = true,
            "--open" => options.open = Some(path("--open")?),
            "--runs" => options.runs = Some(path("--runs")?),
            "--no-save" => options.runs = None,
            "--snapshot" => options.snapshot = Some(path("--snapshot")?),
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(options)
}

/// The shared state, the app on top of it, and whether a key is available.
fn build(options: &Options) -> Result<(Arc<Shared>, App), String> {
    if let Some(dir) = &options.open {
        let state = session::load_run(dir)?;
        let shared = Shared::new(Settings::default());
        *shared.lock() = state;
        let app = App::new(shared.clone(), false, false);
        return Ok((shared, app));
    }
    let (key, endpoint, settings) = if options.mock {
        (
            Some("mock-key".to_owned()),
            Some(mock::serve()?),
            Settings {
                model: "mock/model".into(),
                ..Settings::default()
            },
        )
    } else {
        let key = poker_lab::runner::load_key().ok();
        let settings = Settings {
            brain: if key.is_some() {
                Brain::Model
            } else {
                Brain::StandIn
            },
            ..Settings::default()
        };
        (key, None, settings)
    };
    let has_key = key.is_some();
    let shared = Shared::new(settings);
    session::spawn(
        shared.clone(),
        Env {
            key,
            endpoint,
            runs_root: options.runs.clone(),
        },
    );
    Ok((shared.clone(), App::new(shared, has_key, options.mock)))
}

/// Plays a short mock game without a window and writes one picture per screen; with a saved
/// run, writes pictures of its replay instead.
fn snapshot(dir: &Path, open: Option<&Path>) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let options = Options {
        mock: open.is_none(),
        open: open.map(Path::to_path_buf),
        runs: None,
        snapshot: None,
    };
    let (shared, mut app) = build(&options)?;
    let mut canvas = Canvas::new(W, H);
    let mut shot = |app: &mut App, name: &str, input: &Input| -> Result<(), String> {
        app.frame(&mut canvas, input, 0);
        canvas.save_png(&dir.join(format!("{name}.png")), 2)
    };
    let idle = Input {
        mouse: (-1, -1),
        ..Input::default()
    };
    if open.is_some() {
        let (hand, turn) = {
            let state = shared.lock();
            let hand = (0..state.hands.len())
                .max_by_key(|i| state.hands[*i].analysis.decisions.len())
                .unwrap_or(0);
            let turn = state.hands[hand]
                .frames
                .iter()
                .rposition(|f| f.actor == Some(state.hero))
                .unwrap_or(0);
            (hand, turn)
        };
        app.view = View::Replay {
            hand,
            frame: turn,
            playing: false,
            stepped_ms: 0,
        };
        app.tab = Tab::Mind;
        shot(&mut app, "run-1-mind", &idle)?;
        app.tab = Tab::Hand;
        shot(&mut app, "run-2-hand", &idle)?;
        app.tab = Tab::Session;
        shot(&mut app, "run-3-session", &idle)?;
        app.tab = Tab::Notes;
        shot(&mut app, "run-4-notes", &idle)?;
        println!("wrote 4 pictures to {}", dir.display());
        return Ok(());
    }
    shot(&mut app, "1-setup", &idle)?;
    shared.control(|state| state.speed = 0.0);
    shot(
        &mut app,
        "2-started",
        &Input {
            keys: vec![Key::Enter],
            ..idle.clone()
        },
    )?;
    let deadline = Instant::now() + Duration::from_secs(120);
    while shared.lock().hands.len() < 40 {
        if Instant::now() > deadline {
            return Err("the mock game did not reach 40 hands".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    shared.control(|state| {
        state.paused = true;
        state.speed = 1.0;
    });
    std::thread::sleep(Duration::from_millis(1500));
    shot(&mut app, "3-live-paused", &idle)?;
    // The most instructive hand: the watched seat acted most and reached a showdown.
    let (hand, turn, showdown) = {
        let state = shared.lock();
        let hand = (0..state.hands.len())
            .max_by_key(|i| {
                let a = &state.hands[*i].analysis;
                a.decisions.len() * 2 + usize::from(a.reached_showdown) * 3
            })
            .unwrap_or(0);
        let frames = &state.hands[hand].frames;
        let turn = frames
            .iter()
            .rposition(|f| f.actor == Some(state.hero))
            .unwrap_or(0);
        let showdown = frames.len().saturating_sub(2);
        (hand, turn, showdown)
    };
    let replay = |frame| View::Replay {
        hand,
        frame,
        playing: false,
        stepped_ms: 0,
    };
    app.view = replay(turn);
    app.tab = Tab::Mind;
    shot(&mut app, "4-replay-mind", &idle)?;
    app.tab = Tab::Hand;
    shot(&mut app, "5-replay-hand", &idle)?;
    app.view = replay(showdown);
    shot(
        &mut app,
        "6-showdown",
        &Input {
            text: "h".into(),
            ..idle.clone()
        },
    )?;
    app.tab = Tab::Session;
    shot(&mut app, "7-session", &idle)?;
    // The end of the session panel: the leak report and what the lab measured.
    shot(
        &mut app,
        "7b-session-end",
        &Input {
            mouse: (600, 200),
            wheel: 10_000,
            ..idle.clone()
        },
    )?;
    app.tab = Tab::Hands;
    shot(&mut app, "8-hands", &idle)?;
    app.tab = Tab::Notes;
    shot(&mut app, "8b-notes", &idle)?;
    shot(
        &mut app,
        "9-settings-mid-game",
        &Input {
            text: "lc".into(),
            ..idle.clone()
        },
    )?;
    shared.control(|state| state.quit = true);
    println!("wrote 10 pictures to {}", dir.display());
    Ok(())
}

struct Shell {
    app: App,
    shared: Arc<Shared>,
    canvas: Canvas,
    input: Input,
    wheel: f32,
    cursor: (f64, f64),
    started: Instant,
    next_frame: Instant,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
}
impl Shell {
    /// Largest whole-number scale that fits, and the offset that centres the picture.
    fn layout(&self) -> (i32, i32, i32) {
        let Some(window) = &self.window else {
            return (1, 0, 0);
        };
        let size = window.inner_size();
        let scale = ((size.width as i32 / W).min(size.height as i32 / H)).max(1);
        (
            scale,
            (size.width as i32 - W * scale) / 2,
            (size.height as i32 - H * scale) / 2,
        )
    }

    fn draw(&mut self) {
        let (scale, ox, oy) = self.layout();
        self.input.mouse = (
            ((self.cursor.0 as i32 - ox) as f32 / scale as f32).floor() as i32,
            ((self.cursor.1 as i32 - oy) as f32 / scale as f32).floor() as i32,
        );
        self.input.wheel = self.wheel.trunc() as i32;
        self.wheel -= self.wheel.trunc();
        let ms = self.started.elapsed().as_millis() as u64;
        self.app.frame(&mut self.canvas, &self.input, ms);
        self.input.clicked = false;
        self.input.keys.clear();
        self.input.text.clear();

        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        if surface.resize(width, height).is_err() {
            return;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        let (w, h) = (size.width as usize, size.height as usize);
        buffer.fill(theme::ROOM);
        let scale = scale as usize;
        let mut row = vec![theme::ROOM; W as usize * scale];
        for y in 0..H as usize {
            let source = &self.canvas.px[y * W as usize..(y + 1) * W as usize];
            for (x, colour) in source.iter().enumerate() {
                row[x * scale..(x + 1) * scale].fill(*colour);
            }
            for dy in 0..scale {
                let ty = oy.max(0) as usize + y * scale + dy;
                let tx = ox.max(0) as usize;
                if ty < h && tx + row.len() <= w {
                    buffer[ty * w + tx..ty * w + tx + row.len()].copy_from_slice(&row);
                }
            }
        }
        let _ = buffer.present();
    }
}
impl ApplicationHandler for Shell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Poker Lab Table")
            .with_inner_size(LogicalSize::new(W * 2, H * 2))
            .with_min_inner_size(LogicalSize::new(W, H));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Rc::new(window),
            Err(error) => {
                eprintln!("could not open a window: {error}");
                event_loop.exit();
                return;
            }
        };
        let surface = softbuffer::Context::new(window.clone())
            .and_then(|context| softbuffer::Surface::new(&context, window.clone()));
        match surface {
            Ok(surface) => self.surface = Some(surface),
            Err(error) => {
                eprintln!("could not create a drawing surface: {error}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.shared.control(|state| state.quit = true);
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CursorMoved { position, .. } => self.cursor = (position.x, position.y),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.input.clicked = true;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.wheel -= match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines * 2.0,
                    MouseScrollDelta::PixelDelta(pixels) => pixels.y as f32 / 12.0,
                };
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let named = match &event.logical_key {
                    WinitKey::Named(NamedKey::Space) => Some(Key::Space),
                    WinitKey::Named(NamedKey::ArrowLeft) => Some(Key::Left),
                    WinitKey::Named(NamedKey::ArrowRight) => Some(Key::Right),
                    WinitKey::Named(NamedKey::ArrowUp) => Some(Key::Up),
                    WinitKey::Named(NamedKey::ArrowDown) => Some(Key::Down),
                    WinitKey::Named(NamedKey::Tab) => Some(Key::Tab),
                    WinitKey::Named(NamedKey::Enter) => Some(Key::Enter),
                    WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
                    WinitKey::Named(NamedKey::Backspace) => Some(Key::Backspace),
                    _ => None,
                };
                match named {
                    Some(key) => self.input.keys.push(key),
                    None => {
                        if let Some(text) = &event.text {
                            self.input.text.push_str(text);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_frame {
            self.next_frame = now + Duration::from_millis(33);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

fn main() {
    let result = options().and_then(|options| {
        if let Some(dir) = &options.snapshot {
            return snapshot(dir, options.open.as_deref());
        }
        let (shared, app) = build(&options)?;
        if options.open.is_none() && shared.lock().phase != Phase::Setup {
            return Err("the table did not start in setup".into());
        }
        let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
        let mut shell = Shell {
            app,
            shared,
            canvas: Canvas::new(W, H),
            input: Input::default(),
            wheel: 0.0,
            cursor: (-1.0, -1.0),
            started: Instant::now(),
            next_frame: Instant::now(),
            window: None,
            surface: None,
        };
        event_loop.run_app(&mut shell).map_err(|e| e.to_string())
    });
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(2);
    }
}
