//! Spike/port CLI: perft, parity (count + move order), zobrist verify, bench.

use jungle_lib::{Board, Move, generate_moves, perft, SQUARES};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: jungle-cli <perft|parity|order-parity|zobrist-verify|bench> ...");
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
                let turn: u8 = parts[4].parse().unwrap();
                let grid = hex_grid(parts[3]);
                let b = Board::from_grid(&grid, turn);
                let actual = perft(&b, turn, depth);
                checked += 1;
                if actual != expected {
                    mismatches += 1;
                    eprintln!("MISMATCH {} D{}: expected {}, got {}", parts[0], depth, expected, actual);
                }
            }
            println!("parity: {checked} checked, {mismatches} mismatches");
            std::process::exit(if mismatches == 0 { 0 } else { 1 });
        }
        "order-parity" => {
            let path = args.get(2).expect("dump file required");
            let text = std::fs::read_to_string(path).expect("read dump");
            let mut checked = 0usize;
            let mut mismatches = 0usize;
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                // Format: P{i} hex=<126 hex> stm=<0|1> moves=from>to,from>to,...
                let hex_start = line.find("hex=").map(|i| i + 4).expect("hex=");
                let hex_end = line[hex_start..].find(' ').map(|i| hex_start + i).unwrap();
                let hex = &line[hex_start..hex_end];
                let stm_start = line.find("stm=").map(|i| i + 4).expect("stm=");
                let stm: u8 = line[stm_start..stm_start + 1].parse().unwrap();
                let moves_start = line.find("moves=").map(|i| i + 6).expect("moves=");
                let moves_str = &line[moves_start..];

                let grid = hex_grid(hex);
                let b = Board::from_grid(&grid, stm);
                let mut buf = [Move { from: 0, to: 0, captured_id: 0, enters_den: false }; 128];
                let n = generate_moves(&b, stm, &mut buf);

                let mut actual = String::new();
                for i in 0..n {
                    if i > 0 {
                        actual.push(',');
                    }
                    actual.push_str(&format!("{}>{}", buf[i].from, buf[i].to));
                }
                checked += 1;
                if actual != moves_str {
                    mismatches += 1;
                    eprintln!("ORDER MISMATCH {}:", line.split_whitespace().next().unwrap());
                    eprintln!("  expected: {}", moves_str);
                    eprintln!("  actual  : {}", actual);
                }
            }
            println!("order-parity: {checked} checked, {mismatches} mismatches");
            std::process::exit(if mismatches == 0 { 0 } else { 1 });
        }
        "zobrist-verify" => {
            let path = args.get(2).expect("dump file required");
            let text = std::fs::read_to_string(path).expect("read dump");
            let lines: Vec<&str> = text.lines().collect();
            assert_eq!(lines.len(), 1010, "expected 1010 lines");
            let z = jungle_lib::zobrist::Zobrist::new();
            let mut mismatches = 0usize;
            for p in 0..SQUARES {
                for i in 0..16 {
                    let expected = u64::from_str_radix(lines[p * 16 + i], 16).unwrap();
                    if z.piece_keys[p][i] != expected {
                        mismatches += 1;
                        if mismatches <= 5 {
                            eprintln!("key[{p}][{i}] expected {expected:x} got {:x}", z.piece_keys[p][i]);
                        }
                    }
                }
            }
            let turn_key = u64::from_str_radix(lines[1008], 16).unwrap();
            if z.turn_key != turn_key {
                mismatches += 1;
                eprintln!("turn key mismatch");
            }
            let start_hash = u64::from_str_radix(lines[1009], 16).unwrap();
            let b = Board::initial();
            if b.hash != start_hash {
                mismatches += 1;
                eprintln!("start hash expected {start_hash:x} got {:x}", b.hash);
            }
            println!("zobrist-verify: {mismatches} mismatches");
            std::process::exit(if mismatches == 0 { 0 } else { 1 });
        }
        "bench" => {
            let ms: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
            let b = Board::initial();
            let start = std::time::Instant::now();
            let budget = std::time::Duration::from_millis(ms);
            let mut nodes = 0u64;
            let mut depth = 1u8;
            while start.elapsed() < budget {
                nodes += perft(&b, 0, depth);
                depth += 1;
            }
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            println!("bench: {nodes} nodes in {ms}ms = {:.0} nps", nodes as f64 / elapsed);
        }
        _ => {
            eprintln!("unknown command");
            std::process::exit(1);
        }
    }
}

/// Parses a 126-char hex string into a 63-byte grid (square -> id = type+1).
fn hex_grid(hex: &str) -> [u8; SQUARES] {
    assert_eq!(hex.len(), 126, "hex layout must be 126 chars");
    let mut grid = [0u8; SQUARES];
    for sq in 0..SQUARES {
        let b = u8::from_str_radix(&hex[sq * 2..sq * 2 + 2], 16).unwrap();
        grid[sq] = b;
    }
    grid
}
