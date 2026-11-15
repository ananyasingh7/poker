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
        let paths: Vec<_> = args[1..]
            .iter()
            .filter(|a| *a != "--json" && *a != "--no-grades")
            .collect();
        let [path] = paths.as_slice() else {
            return Err(
                "usage: poker-lab analyze RUN_OR_BATCH_DIRECTORY [--json] [--no-grades]".into(),
            );
        };
        let analysis = poker_lab::analyze::directory_with(Path::new(path), !no_grades)?;
        if json_output {
            println!("{}", serde_json::to_string_pretty(&analysis)?);
        } else {
            print!("{}", poker_lab::analyze::render(&analysis));
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
            "poker-lab [SEATS=6] [HANDS=10] [SEED=1] [NEW_LOG_DIRECTORY]\npoker-lab replay PATH/evaluator.jsonl\npoker-lab openrouter CONFIG.json NEW_LOG_DIRECTORY\npoker-lab report RUN_DIRECTORY [PRIVATE_SEAT]\npoker-lab batch BATCH_CONFIG.json NEW_LOG_DIRECTORY [--resume: continue an interrupted batch in that directory]\npoker-lab bench [SESSIONS=100] [HANDS=10] [NEW_RESULT.json]   compare in-process and subprocess scripted tables\npoker-lab analyze RUN_OR_BATCH_DIRECTORY [--json] [--no-grades]   playing style, cost per hand, win rates with seed-block intervals, all-in adjusted results and pot-odds decision grades\npoker-lab ceiling [FIELD=default|style,style,...] [TRAIN_SEEDS=8] [TEST_SEEDS=40] [NEW_RESULT.json]   exploitation ceiling against a scripted field\npoker-lab participant   internal worker (participant protocol v2 over stdin/stdout)\nSet \"execution\": \"subprocess\" in a model config to run one supervised worker process per seat."
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
