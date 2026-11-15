// ABOUTME: A local stand-in for the OpenRouter endpoint, for tests and the offline `--mock` demo:
// ABOUTME: answers action and notebook requests with legal moves and canned reasoning. Never used for results.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
};

/// Starts the mock on a free local port and returns its chat-completions URL.
pub fn serve() -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = answer(stream);
        }
    });
    Ok(format!("http://{address}/api/v1/chat/completions"))
}

fn answer(mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let reply = completion(&request).to_string();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    )?;
    stream.flush()
}

fn rank(card: &Value) -> usize {
    card.as_str()
        .and_then(|c| c.chars().next())
        .and_then(|r| "23456789TJQKA".find(r))
        .unwrap_or(0)
}

/// The mock's reply to one chat-completions request.
pub fn completion(request: &Value) -> Value {
    let observation: Value = request["messages"][1]["content"]
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(Value::Null);
    let schema = request["response_format"]["json_schema"]["name"]
        .as_str()
        .unwrap_or_default();
    let reasons = request["reasoning"]["enabled"] == true;
    let (content, reasoning) = if schema.starts_with("notebook_profiles") {
        let seat = observation["button"].as_u64().unwrap_or(1).max(1);
        (
            json!({"operation":"replace","profiles":[{"seat":seat,"profile":format!(
                "Mock note after {} events: entered the pot, no showdown seen yet (1 hand, tentative).",
                observation["history"].as_array().map_or(0, Vec::len))}]}),
            "Nothing decisive this hand; record one tentative line.".to_owned(),
        )
    } else if schema.starts_with("notebook") {
        (json!({"operation":"keep","notebook":""}), String::new())
    } else {
        let legal = &observation["legal"];
        let (high, low) = {
            let (a, b) = (rank(&observation["hole"][0]), rank(&observation["hole"][1]));
            (a.max(b), a.min(b))
        };
        let strong = high == low || (high >= 11 && low >= 8);
        let playable = strong || high >= 10 || low >= 7;
        let to_call = legal["call"].as_u64();
        let hole = format!(
            "{} {}",
            observation["hole"][0].as_str().unwrap_or("?"),
            observation["hole"][1].as_str().unwrap_or("?")
        );
        let (action, amount, why) = match (to_call, legal["min_raise_to"].as_u64()) {
            (None, Some(min)) if strong => (
                "raise_to",
                Some(min),
                "Nobody has bet and my hand is strong, so I bet the minimum to build the pot.",
            ),
            (None, _) => (
                "check",
                None,
                "No bet to face and nothing worth building a pot with: checking is free.",
            ),
            (Some(_), Some(min)) if strong && high >= 12 => (
                "raise_to",
                Some(min),
                "A premium holding facing a bet. Raising the minimum charges weaker hands.",
            ),
            (Some(call), _) if playable && call <= observation["pot"].as_u64().unwrap_or(0) => (
                "call",
                None,
                "The price is fair against the pot and my cards can improve, so I call.",
            ),
            (Some(_), _) => (
                "fold",
                None,
                "The bet is too large for this holding. I give it up.",
            ),
        };
        (
            json!({"action":action,"amount":amount}),
            format!(
                "I hold {hole}. The pot is {} and {}. {why}",
                observation["pot"],
                to_call.map_or("there is no bet to me".to_owned(), |c| format!(
                    "it costs {c} to call"
                )),
            ),
        )
    };
    let prompt_tokens = request.to_string().len() as u64 / 4;
    let reasoning_tokens = if reasons {
        reasoning.len() as u64 / 4
    } else {
        0
    };
    json!({"id":"mock-generation","model":request["model"],"provider":"Mock",
        "choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant",
            "content":content.to_string(),"reasoning":if reasons && !reasoning.is_empty() { json!(reasoning) } else { Value::Null }}}],
        "usage":{"prompt_tokens":prompt_tokens,"completion_tokens":12 + reasoning_tokens,"cost":0.0,
            "completion_tokens_details":{"reasoning_tokens":reasoning_tokens}}})
}
