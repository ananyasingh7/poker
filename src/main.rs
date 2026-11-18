use poker_lab::{
    arena::{Agent, Record, Scripted, Session, replay},
    cards::Rng,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};
fn create(path: impl AsRef<Path>) -> Result<BufWriter<File>, Box<dyn std::error::Error>> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(BufWriter::new(options.open(path)?))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "participant") {
        // Internal worker entry point: protocol frames only on stdout, diagnostics on stderr.
        if args.len() != 1 {
            return Err("usage: poker-lab participant (internal; speaks participant protocol v2 on stdin/stdout)".into());
        }
        let code = poker_lab::worker::run(std::io::stdin().lock(), std::io::stdout().lock());
        std::process::exit(code);
    }
    if args.first().is_some_and(|a| a == "bench") {
        return bench(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "analyze") {
        let json_output = args.iter().any(|a| a == "--json");
        let no_grades = args.iter().any(|a| a == "--no-grades");
        let per_decision = args.iter().any(|a| a == "--decisions");
        let turn = args.iter().any(|a| a == "--turn");
        let paths: Vec<_> = args[1..]
            .iter()
            .filter(|a| {
                !matches!(
                    a.as_str(),
                    "--json" | "--no-grades" | "--decisions" | "--turn"
                )
            })
            .collect();
        let [path] = paths.as_slice() else {
            return Err(
                "usage: poker-lab analyze RUN_OR_BATCH_DIRECTORY [--json] [--no-grades] [--decisions] [--turn]"
                    .into(),
            );
        };
        if per_decision {
            // One JSON object per line, for jq or pandas.
            let mut out = std::io::stdout().lock();
            for row in poker_lab::analyze::decisions(Path::new(path))? {
                match writeln!(out, "{row}") {
                    // A closed pipe (`| head`) is the reader's choice, not an error.
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => return Ok(()),
                    other => other?,
                }
            }
            return Ok(());
        }
        let analysis = poker_lab::analyze::directory_graded(Path::new(path), !no_grades, turn)?;
        if json_output {
            println!("{}", serde_json::to_string_pretty(&analysis)?);
        } else {
            print!("{}", poker_lab::analyze::render(&analysis));
        }
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "compare") {
        let json_output = args.iter().any(|a| a == "--json");
        let turn = args.iter().any(|a| a == "--turn");
        let usage = "usage: poker-lab compare BATCH_DIRECTORY... [--seat N] [--turn] [--json] [--save FILE.json]";
        let mut seat = None;
        let mut save = None;
        let mut dirs: Vec<&Path> = vec![];
        let mut rest = args[1..].iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--json" | "--turn" => {}
                "--seat" => seat = Some(rest.next().ok_or(usage)?.parse::<usize>()?),
                "--save" => save = Some(rest.next().ok_or(usage)?),
                dir => dirs.push(Path::new(dir)),
            }
        }
        if dirs.is_empty() {
            return Err(usage.into());
        }
        let comparison = poker_lab::compare::compare(&dirs, turn, seat)?;
        if let Some(file) = save {
            // The full result (per-seed values included) beside the table, for later reading.
            std::fs::write(file, serde_json::to_string_pretty(&comparison)?)?;
        }
        if json_output {
            println!("{}", serde_json::to_string_pretty(&comparison)?);
        } else {
            print!("{}", poker_lab::compare::render(&comparison));
        }
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "ceiling") {
        return ceiling(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "report") {
        if !(2..=3).contains(&args.len()) {
            return Err("usage: poker-lab report RUN_DIRECTORY [PRIVATE_SEAT]".into());
        }
        print!(
            "{}",
            poker_lab::report::directory(
                Path::new(&args[1]),
                args.get(2).map(|s| s.parse()).transpose()?
            )?
        );
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "batch") {
        return batch(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "openrouter") {
        return pilot(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "replay") {
        if args.len() != 2 {
            return Err("usage: poker-lab replay PATH/evaluator.jsonl".into());
        }
        let mut count = 0;
        for line in BufReader::new(File::open(&args[1])?).lines() {
            let record: Record = serde_json::from_str(&line?)?;
            replay(&record)?;
            count += 1;
        }
        println!("Verified {count} hands");
        return Ok(());
    }
    if args.iter().any(|a| a == "--help") {
        println!(
            "poker-lab [SEATS=6] [HANDS=10] [SEED=1] [NEW_LOG_DIRECTORY]\npoker-lab replay PATH/evaluator.jsonl\npoker-lab openrouter CONFIG.json NEW_LOG_DIRECTORY\npoker-lab report RUN_DIRECTORY [PRIVATE_SEAT]\npoker-lab batch BATCH_CONFIG.json NEW_LOG_DIRECTORY [--resume: continue an interrupted batch in that directory]\npoker-lab bench [SESSIONS=100] [HANDS=10] [NEW_RESULT.json]   compare in-process and subprocess scripted tables\npoker-lab analyze RUN_OR_BATCH_DIRECTORY [--json] [--no-grades]   playing style, cost per hand, win rates with seed-block intervals, all-in adjusted results and pot-odds decision grades\n    --decisions: instead, one JSON line per model decision (spot, action, equity against the real cards, pot odds, bettor)\n    --turn: also grade turn decisions approximately against the bots (slower; see docs/17)\npoker-lab compare BATCH_DIRECTORY... [--seat N] [--turn] [--json] [--save FILE.json]   arms of several runs side by side (model · thinking · condition), paired on shared complete seeds; the first directory's model is the reference\npoker-lab ceiling [FIELD=default|style,style,...] [TRAIN_SEEDS=8] [TEST_SEEDS=40] [NEW_RESULT.json]   exploitation ceiling against a scripted field\npoker-lab participant   internal worker (participant protocol v2 over stdin/stdout)\nSet \"execution\": \"subprocess\" in a model config to run one supervised worker process per seat."
        );
        return Ok(());
    }
    if args.len() > 4 {
        return Err("too many arguments; use --help".into());
    }
    let seats = args.first().map(|s| s.parse()).transpose()?.unwrap_or(6);
    let hands: usize = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(10);
    let seed: u64 = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(1);
    let mut session = Session::new(seats, seed)?;
    let mut agents: Vec<Box<dyn Agent>> = (0..seats)
        .map(|i| Box::new(Scripted::Random(Rng(seed.wrapping_add(i as u64)))) as Box<dyn Agent>)
        .collect();
    let mut logs = if let Some(dir) = args.get(3) {
        fs::create_dir(dir)?;
        let dir = Path::new(dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
        let private = (0..seats)
            .map(|i| create(dir.join(format!("seat-{i}.jsonl"))))
            .collect::<Result<Vec<_>, _>>()?;
        Some((
            create(dir.join("public.jsonl"))?,
            create(dir.join("evaluator.jsonl"))?,
            private,
        ))
    } else {
        None
    };
    for index in 0..hands {
        let played = session.next_hand(&mut agents)?;
        println!(
            "Hand {} button={} decisions={} stacks={:?}",
            index + 1,
            played.record.button,
            played.record.decisions.len(),
            session.stacks
        );
        if let Some((public, evaluator, private)) = &mut logs {
            serde_json::to_writer(&mut *public, &played.record.events)?;
            writeln!(public)?;
            serde_json::to_writer(&mut *evaluator, &played.record)?;
            writeln!(evaluator)?;
            for (file, views) in private.iter_mut().zip(&played.private_views) {
                serde_json::to_writer(&mut *file, views)?;
                writeln!(file)?;
            }
        }
    }
    if let Some((mut public, mut evaluator, private)) = logs {
        public.flush()?;
        evaluator.flush()?;
        for mut f in private {
            f.flush()?;
        }
    }
    println!("Rebuy chips by seat: {:?}", session.rebuys);
    Ok(())
}

fn pilot(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use poker_lab::{openrouter, runner};
    if args.len() != 2 {
        return Err("usage: poker-lab openrouter CONFIG.json NEW_LOG_DIRECTORY".into());
    }
    let config: openrouter::Config = serde_json::from_reader(File::open(&args[0])?)?;
    if config.execution == openrouter::Execution::Subprocess {
        poker_lab::supervisor::install_interrupt_cleanup();
    }
    runner::session(
        config,
        Path::new(&args[1]),
        &runner::Env::new(runner::load_key()?),
    )?;
    Ok(())
}

fn batch(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use poker_lab::runner;
    let resume = args.len() == 3 && args[2] == "--resume";
    if args.len() != 2 && !resume {
        return Err("usage: poker-lab batch CONFIG.json NEW_LOG_DIRECTORY | poker-lab batch CONFIG.json EXISTING_BATCH_DIRECTORY --resume".into());
    }
    let config: poker_lab::experiment::Batch = serde_json::from_reader(File::open(&args[0])?)?;
    let run = if resume {
        runner::resume
    } else {
        runner::batch
    };
    let result = run(
        &config,
        Path::new(&args[1]),
        &runner::Env::new(runner::load_key()?),
    )?;
    println!(
        "Batch complete: {} of {} sessions, {} requests, ${:.6}{}",
        result["completed_sessions"],
        result["scheduled_sessions"],
        result["requests"],
        result["reported_cost_usd"].as_f64().unwrap_or(0.0),
        result["stop_reason"]
            .as_str()
            .map(|r| format!(" (stopped: {r})"))
            .unwrap_or_default()
    );
    println!("Next: poker-lab analyze {}", args[1]);
    Ok(())
}

fn bench(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() > 3 {
        return Err("usage: poker-lab bench [SESSIONS=100] [HANDS=10] [NEW_RESULT.json]".into());
    }
    let sessions: usize = args.first().map(|s| s.parse()).transpose()?.unwrap_or(100);
    let hands: usize = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(10);
    poker_lab::supervisor::install_interrupt_cleanup();
    let result = poker_lab::bench::run(
        sessions,
        hands,
        &poker_lab::supervisor::Launch::current_exe()?,
    )?;
    let text = serde_json::to_string_pretty(&result)?;
    if let Some(path) = args.get(2) {
        let mut file = create(path)?;
        writeln!(file, "{text}")?;
        file.flush()?;
    }
    println!("{text}");
    Ok(())
}

fn ceiling(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use poker_lab::{ceiling, opponents};
    if args.len() > 4 {
        return Err(
            "usage: poker-lab ceiling [FIELD] [TRAIN_SEEDS=8] [TEST_SEEDS=40] [NEW_RESULT.json]"
                .into(),
        );
    }
    let field = match args.first().map(String::as_str) {
        None | Some("default") => opponents::DEFAULT_FIELD.to_vec(),
        Some(list) => list
            .split(',')
            .map(|name| serde_json::from_value(serde_json::Value::String(name.trim().into())))
            .collect::<Result<Vec<opponents::Style>, _>>()
            .map_err(|e| format!("unknown opponent style: {e}"))?,
    };
    let search = ceiling::Search {
        field,
        train_seeds: args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(8),
        test_seeds: args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(40),
        hands: 60,
        threads: std::thread::available_parallelism().map_or(4, |n| n.get()),
    };
    let text = serde_json::to_string_pretty(&ceiling::run(&search)?)?;
    if let Some(path) = args.get(3) {
        let mut file = create(path)?;
        writeln!(file, "{text}")?;
        file.flush()?;
    }
    println!("{text}");
    Ok(())
}
