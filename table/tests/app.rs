// ABOUTME: Drives the spectator screen without a window: starting from the settings sheet,
// ABOUTME: pausing, replaying a finished hand and returning to the live game, on every panel tab.
use poker_table::{
    app::{App, Input, Key, Tab, View},
    canvas::{Canvas, H, W},
    session::{self, Brain, Env, Phase, Settings, Shared},
};
use std::time::{Duration, Instant};

fn keys(keys: &[Key], text: &str) -> Input {
    Input {
        mouse: (-1, -1),
        keys: keys.to_vec(),
        text: text.into(),
        ..Input::default()
    }
}

fn wait(shared: &Shared, what: &str, done: impl Fn(&session::State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !done(&shared.lock()) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn spectator_can_start_pause_replay_and_return_to_live() {
    let shared = Shared::new(Settings {
        brain: Brain::StandIn,
        ..Settings::default()
    });
    session::spawn(
        shared.clone(),
        Env {
            key: None,
            endpoint: None,
            runs_root: None,
        },
    );
    let mut app = App::new(shared.clone(), false, false);
    let mut canvas = Canvas::new(W, H);
    let idle = keys(&[], "");

    // Nothing is dealt until the spectator starts the game from the settings sheet.
    app.frame(&mut canvas, &idle, 0);
    assert_eq!(shared.lock().phase, Phase::Setup);
    assert!(shared.lock().live.is_none());
    shared.control(|state| state.speed = 0.0);
    app.frame(&mut canvas, &keys(&[Key::Enter], ""), 0);
    assert_eq!(shared.lock().phase, Phase::Playing);
    wait(&shared, "five hands", |s| s.hands.len() >= 5);

    // Space pauses: no further hand completes while paused.
    app.frame(&mut canvas, &keys(&[Key::Space], ""), 0);
    assert!(shared.lock().paused);
    std::thread::sleep(Duration::from_millis(300));
    let paused_at = shared.lock().hands.len();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(shared.lock().hands.len(), paused_at);

    // Every tab draws something, live and in replay, with all cards shown or hidden.
    for text in ["", "h"] {
        for tab in [Tab::Mind, Tab::Hand, Tab::Session, Tab::Hands, Tab::Notes] {
            app.tab = tab;
            app.frame(&mut canvas, &keys(&[], text), 0);
            assert!(canvas.px.iter().any(|c| *c != canvas.px[0]));
        }
    }

    // Space resumes; stepping back opens the last finished hand and holds the live game.
    app.frame(&mut canvas, &keys(&[Key::Space], ""), 0);
    assert!(!shared.lock().paused);
    app.frame(&mut canvas, &keys(&[Key::Left], ""), 0);
    assert!(matches!(app.view, View::Replay { .. }));
    assert!(shared.lock().paused, "replay holds the live game");
    let View::Replay { hand, frame, .. } = app.view else {
        unreachable!()
    };
    app.frame(&mut canvas, &keys(&[Key::Left, Key::Left], ""), 0);
    let View::Replay {
        hand: same,
        frame: earlier,
        ..
    } = app.view
    else {
        unreachable!()
    };
    assert_eq!((same, earlier + 2), (hand, frame));
    app.frame(&mut canvas, &keys(&[Key::Up], ""), 0);
    let View::Replay {
        hand: previous,
        frame: first,
        ..
    } = app.view
    else {
        unreachable!()
    };
    assert_eq!((previous + 1, first), (hand, 0));
    for tab in [Tab::Mind, Tab::Hand, Tab::Session, Tab::Hands, Tab::Notes] {
        app.tab = tab;
        app.frame(&mut canvas, &idle, 0);
    }

    // Returning to live resumes the game it interrupted.
    let before = shared.lock().hands.len();
    app.frame(&mut canvas, &keys(&[], "l"), 0);
    assert_eq!(app.view, View::Live);
    assert!(!shared.lock().paused);
    wait(&shared, "play to continue", |s| s.hands.len() > before);

    // Opening the settings pauses; cancelling leaves the settings alone.
    app.frame(&mut canvas, &keys(&[], "c"), 0);
    assert!(shared.lock().paused);
    app.frame(&mut canvas, &keys(&[Key::Escape], ""), 0);
    assert_eq!(shared.lock().settings.brain, Brain::StandIn);
    shared.control(|state| state.quit = true);
}
