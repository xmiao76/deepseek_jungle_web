//! Spike CLI: perft from the start position, parity verification against a
//! C# oracle dump, and a native bench (nodes at a fixed time budget).

use jungle_lib::{perft, Board};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: jungle-cli <perft <depth> | parity <dump> | bench <ms>>");
        std::process::exit(1);
    }
    match args[1].as_str() {
        "perft" => {
            let depth: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);
            let b = Board::initial();
            let nodes = perft(&b, 0, depth);
            println!("perft({depth}) = {nodes}");
        }
        "parity" => {
            // Dump format (from the C# --perft-dump mode):
            //   P<index> D<depth> <count> <hex:63 types> <turn>
            let path = args.get(2).expect("dump file required");
            let text = std::fs::read_to_string(path).expect("read dump");
            let mut checked = 0usize;
            let mut mismatches = 0usize;
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() != 5 {
                    eprintln!("bad dump line: {line}");
                    mismatches += 1;
                    continue;
                }
                let depth: u8 = parts[1].trim_start_matches('D').parse().unwrap();
                let expected: u64 = parts[2].parse().unwrap();
                let hex = parts[3];
                let turn: u8 = parts[4].parse().unwrap();

                let mut types = Vec::new();
                let mut squares = Vec::new();
                for (sq, chunk) in hex.as_bytes().chunks(2).enumerate() {
                    let id = u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap();
                    if id != 0 {
                        types.push(id - 1);
                        squares.push(sq as u8);
                    }
                }
                let b = Board::from_packed(&types, &squares, turn);
                let actual = perft(&b, turn, depth);
                checked += 1;
                if actual != expected {
                    mismatches += 1;
                    eprintln!("MISMATCH {} D{}: expected {}, got {}", parts[0], depth, expected, actual);
                }
            }
            println!("parity: {checked} positions checked, {mismatches} mismatches");
            std::process::exit(if mismatches == 0 { 0 } else { 1 });
        }
        "bench" => {
            let ms: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
            let b = Board::initial();
            let deadline = Instant::now() + std::time::Duration::from_millis(ms);
            let mut nodes = 0u64;
            // Fixed-depth iterations until the deadline (mirrors the C# bench's
            // iterative deepening; perft-style counting is order-equivalent for
            // the nps measurement).
            let mut depth = 1u8;
            loop {
                if Instant::now() >= deadline {
                    break;
                }
                nodes += perft(&b, 0, depth);
                depth += 1;
            }
            let elapsed = deadline.elapsed().as_secs_f64().max(0.001);
            println!("bench: {nodes} nodes in ~{ms}ms = {:.0} nps", nodes as f64 / elapsed);
        }
        _ => {
            eprintln!("unknown command");
            std::process::exit(1);
        }
    }
}
