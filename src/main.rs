use clap::Parser;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use solana_vanity::{grind, validate_prefix, get_difficulty_estimate, GrinderEvent};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 1-6 characters to match at the start of the vanity address
    #[arg(short, long)]
    prefix: String,

    /// Number of matching wallets to find before stopping
    #[arg(short, long, default_value_t = 1)]
    count: usize,

    /// Number of threads to use (default: all cores)
    #[arg(short, long)]
    threads: Option<usize>,

    /// Optional file to append results as JSON lines
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    if let Err(e) = validate_prefix(&args.prefix) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    if args.prefix.len() > 5 {
        println!("⚠ Warning: A {} character prefix may take days to find.", args.prefix.len());
    }

    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().unwrap();
    }

    let core_count = rayon::current_num_threads();
    let (time_est, _) = get_difficulty_estimate(args.prefix.len());

    println!("Searching for prefix: '{}'", args.prefix);
    println!("Target count: {}", args.count);
    println!("Threads: {}", core_count);
    println!("Estimated time: {}", time_est);
    println!("--------------------------------------------------");

    let (tx, rx) = mpsc::channel();
    let abort_signal = Arc::new(AtomicBool::new(false));

    // Start grinder in a detached thread because we block on rx receiving events
    let prefix = args.prefix.clone();
    let count = args.count;
    let abort_signal_clone = abort_signal.clone();
    std::thread::spawn(move || {
        grind(prefix, count, tx, abort_signal_clone);
    });

    for event in rx {
        match event {
            GrinderEvent::Progress(p) => {
                println!(
                    "[{:.1}s]    {:>10} attempts | {:>8.0} keys/sec | {}/{} found",
                    p.elapsed, p.attempts, p.rate, p.found, p.needed
                );
            }
            GrinderEvent::Match(m) => {
                println!("╔══════════════════════════════════════════════════════╗");
                println!("║  MATCH FOUND                                         ║");
                println!("╠══════════════════════════════════════════════════════╣");
                println!("║  Public Key:   {:<37} ║", m.public_key);
                println!("║  Private Key:  {:<37} ║", format!("{}...", &m.private_key[0..10]));
                println!("║                (base58 — import to Phantom)          ║");
                println!("║  Attempts:     {:<37} ║", m.attempts);
                println!("╚══════════════════════════════════════════════════════╝");
                println!("(Full private key: {})", m.private_key);

                if let Some(ref out_path) = args.output {
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(out_path) {
                        let json = serde_json::json!({
                            "public_key": m.public_key,
                            "private_key": m.private_key,
                            "prefix": args.prefix,
                            "attempts": m.attempts,
                            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
                        });
                        let _ = writeln!(file, "{}", json.to_string());
                    } else {
                        eprintln!("Failed to open output file: {:?}", out_path);
                    }
                }
            }
            GrinderEvent::Done { total_attempts, elapsed, found } => {
                let rate = if elapsed > 0.0 { total_attempts as f64 / elapsed } else { 0.0 };
                println!("--------------------------------------------------");
                println!("Finished!");
                println!("Found: {}/{}", found, args.count);
                println!("Total time: {:.1}s", elapsed);
                println!("Total attempts: {}", total_attempts);
                println!("Average rate: {:.0} keys/sec", rate);
                break;
            }
        }
    }
}
