// ABOUTME: Test fixture only: a participant worker that speaks protocol v2 correctly until a named
// ABOUTME: fault fires, so supervisor tests can exercise hangs, bad frames, crashes and ignored shutdowns.
use poker_lab::{
    engine::Action,
    participant::{Message, Outcome, Request, Response, Usage},
    worker::read_frame,
};
use std::{io::Write, thread::sleep, time::Duration};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fault = args.get(1).map(String::as_str).unwrap_or("none");
    // Number of decisions answered correctly before the fault fires.
    let mut healthy: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(0);
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let forever = || sleep(Duration::from_secs(3600));
    while let Ok(Some(frame)) = read_frame(&mut input, 1 << 20) {
        let request: Request = serde_json::from_str(&frame).expect("well-formed host request");
        let mut response = Response {
            identity: request.identity.clone(),
            outcome: Outcome::Ack,
            usage: Usage::default(),
        };
        let mut trailer = String::new();
        match &request.message {
            Message::Init { .. } => {
                if fault == "init_hang" {
                    forever();
                }
                response.outcome = Outcome::Ready {
                    crate_version: "fixture".into(),
                };
            }
            Message::StartHand => {}
            Message::HandEnd { .. } => {
                if fault == "crash_on_hand_end" {
                    std::process::exit(9);
                }
            }
            Message::Shutdown => match fault {
                "ignore_shutdown" => forever(),
                "ack_no_exit" => {
                    writeln!(output, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                    output.flush().unwrap();
                    forever();
                }
                _ => {
                    writeln!(output, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                    return;
                }
            },
            Message::Decide { view, .. } => {
                let legal = view.legal.as_ref().expect("pending decision");
                response.outcome = Outcome::Action {
                    action: if legal.check {
                        Action::Check
                    } else {
                        Action::Call
                    },
                };
                if healthy > 0 {
                    healthy -= 1;
                } else {
                    match fault {
                        "hang" => forever(),
                        "partial_line" => {
                            write!(output, "{{\"identity\":").unwrap();
                            output.flush().unwrap();
                            forever();
                        }
                        "oversized" => {
                            writeln!(output, "{}", "x".repeat(2 << 20)).unwrap();
                            output.flush().unwrap();
                            forever();
                        }
                        "stderr_flood" => {
                            let noise = "e".repeat(64 * 1024);
                            for _ in 0..40 {
                                eprint!("{noise}");
                            }
                        }
                        "delayed" => sleep(Duration::from_millis(1500)),
                        "wrong_id" => response.identity.id += 1,
                        "wrong_seat" => response.identity.seat += 1,
                        "wrong_hand" => response.identity.hand = Some(99),
                        "eof" => std::process::exit(0),
                        "crash" => std::process::exit(7),
                        "garbage" => {
                            writeln!(output, "not json").unwrap();
                            output.flush().unwrap();
                            continue;
                        }
                        "invalid_utf8" => {
                            output.write_all(&[0xff, 0xfe, b'\n']).unwrap();
                            output.flush().unwrap();
                            continue;
                        }
                        "extra_field" => {
                            let mut value = serde_json::to_value(&response).unwrap();
                            value["seed"] = 42.into();
                            writeln!(output, "{value}").unwrap();
                            output.flush().unwrap();
                            continue;
                        }
                        "unsolicited" => trailer = "{\"unsolicited\":true}\n".into(),
                        "overclaim" => response.usage.attempts = 2,
                        "unexpected" => response.outcome = Outcome::Ack,
                        _ => {}
                    }
                }
            }
        }
        // One write, so a trailing unsolicited line is already buffered at the host.
        let line = serde_json::to_string(&response).unwrap() + "\n" + &trailer;
        output.write_all(line.as_bytes()).unwrap();
        output.flush().unwrap();
    }
}
