using System.Text.Json;
using JungleGame.Core.AI;
using JungleGame.Core.Engine;
using JungleGame.Core.Model;

namespace JungleGame.Bench;

/// <summary>
/// Node-count regression bench: fixed-depth searches over checked-in positions
/// (Bench/bench-positions.tsuite) compared against Bench/bench-baseline.json.
/// Node counts at a fixed depth are deterministic (no time dependence, fresh
/// engine per position), so the baseline is machine-independent. Fails when any
/// position's node count grows more than 15% at the same depth — the gate that
/// catches ordering/pruning regressions cheaply.
/// </summary>
internal static class BenchPositions
{
    internal const double NodeGrowthLimit = 1.15;
    internal const string DefaultPositionsPath = "bench-positions.tsuite";
    internal const string DefaultBaselinePath = "bench-baseline.json";

    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    private sealed record PositionBaseline(int Line, int Depth, long Nodes);
    private sealed record BenchBaseline(int FormatVersion, List<PositionBaseline> Positions);

    internal static int Run(string[] args)
    {
        string positionsPath = Args.ResolveDataPath(Args.ReadString(args, "--positions", DefaultPositionsPath) ?? DefaultPositionsPath);
        string baselinePath = Args.ResolveDataPath(Args.ReadString(args, "--baseline", DefaultBaselinePath) ?? DefaultBaselinePath);
        bool writeBaseline = args.Contains("--write-baseline");

        if (!File.Exists(positionsPath))
        {
            Console.Error.WriteLine($"Positions file not found: {positionsPath}");
            return 1;
        }

        var entries = TestSuiteParser.ParseFile(positionsPath);
        var measured = new List<PositionBaseline>(entries.Count);
        Console.WriteLine($"{"Line",4} {"Depth",5} {"Nodes",12} {"Nodes/sec",12}");
        foreach (var entry in entries)
        {
            var engine = new MinimaxEngine(TimeSpan.FromMilliseconds(10_000), entry.SearchDepth);
            var sw = System.Diagnostics.Stopwatch.StartNew();
            var move = engine.FindBestMove(entry.State);
            sw.Stop();

            if (move == null)
            {
                Console.Error.WriteLine($"Line {entry.LineNumber}: no legal move — position unusable for the node bench.");
                return 1;
            }

            long nodes = engine.NodesSearched;
            Console.WriteLine($"{entry.LineNumber,4} {entry.SearchDepth,5} {nodes,12:N0} {nodes / Math.Max(0.001, sw.Elapsed.TotalSeconds),12:N0}");
            measured.Add(new PositionBaseline(entry.LineNumber, entry.SearchDepth, nodes));
        }

        if (writeBaseline)
        {
            WriteBaseline(baselinePath, measured);
            Console.WriteLine($"Baseline written to {baselinePath} ({measured.Count} positions).");
            return 0;
        }

        if (!File.Exists(baselinePath))
        {
            Console.Error.WriteLine($"Baseline missing: {baselinePath} — run with --write-baseline and commit the file.");
            return 1;
        }

        var baseline = JsonSerializer.Deserialize<BenchBaseline>(File.ReadAllText(baselinePath))
            ?? throw new InvalidOperationException($"Unreadable baseline {baselinePath}.");
        if (baseline.FormatVersion != 1)
        {
            Console.Error.WriteLine($"Unsupported baseline format version {baseline.FormatVersion}.");
            return 1;
        }

        bool anyRegression = false;
        foreach (var m in measured)
        {
            var b = baseline.Positions.FirstOrDefault(p => p.Line == m.Line)
                ?? throw new InvalidOperationException(
                    $"Baseline has no entry for line {m.Line}; regenerate with --write-baseline.");
            if (b.Depth != m.Depth)
                throw new InvalidOperationException(
                    $"Baseline depth mismatch on line {m.Line} ({b.Depth} vs {m.Depth}); regenerate with --write-baseline.");

            double ratio = (double)m.Nodes / b.Nodes;
            if (ratio > NodeGrowthLimit)
            {
                anyRegression = true;
                Console.WriteLine($"REGRESSION line {m.Line}: {m.Nodes:N0} vs baseline {b.Nodes:N0} (+{(ratio - 1) * 100:F1}%)");
            }
        }

        if (anyRegression)
        {
            Console.WriteLine($"Node-count regression gate FAILED (> {NodeGrowthLimit:P0} growth at equal depth).");
            return 1;
        }

        Console.WriteLine($"Node-count regression gate passed (≤ {NodeGrowthLimit:P0} growth at equal depth).");
        return 0;
    }

    private static void WriteBaseline(string path, List<PositionBaseline> positions)
    {
        var baseline = new BenchBaseline(1, positions);
        File.WriteAllText(path, JsonSerializer.Serialize(baseline, JsonOptions));
    }

    /// <summary>
    /// Deterministic node-count/best-move dump over the suite positions (depths
    /// 1..8) and seeded fuzz positions (depths 1..4). Fixed-depth searches never
    /// abort (10s budget), so a behavior-neutral refactor must produce a
    /// byte-identical dump — the strongest neutrality gate in the program
    /// (scripts/check-exact-nodes.ps1 diffs two binaries' dumps).
    /// </summary>
    internal static int RunDump(string[] args)
    {
        string outPath = Args.ReadString(args, "--dump-nodes")
            ?? throw new InvalidOperationException("--dump-nodes <file> is required.");
        string positionsPath = Args.ResolveDataPath(Args.ReadString(args, "--positions", DefaultPositionsPath) ?? DefaultPositionsPath);
        int fuzzCount = Args.ReadInt(args, "--fuzz", 200);
        int fuzzSeed = Args.ReadInt(args, "--seed", 20240816);

        if (!File.Exists(positionsPath))
        {
            Console.Error.WriteLine($"Positions file not found: {positionsPath}");
            return 1;
        }

        var entries = TestSuiteParser.ParseFile(positionsPath);
        using var writer = new StreamWriter(outPath);

        // Warm-up search: the first search of the process JIT-compiles the engine,
        // which can blow a fixed wall-clock budget and make early node counts
        // timing-dependent. The dump must be logic-deterministic, not timing-free.
        DumpSearch(null, "W", GameState.CreateInitial(), 2);

        for (int i = 0; i < entries.Count; i++)
        {
            for (int depth = 1; depth <= 8; depth++)
                DumpSearch(writer, $"S{i}", entries[i].State, depth);
        }

        int dumped = 0;
        for (int f = 0; f < fuzzCount; f++)
        {
            var rng = new Random(fuzzSeed * 397 + f);
            int plies = 1 + rng.Next(60);
            var state = GameState.CreateInitial();
            bool live = true;
            for (int p = 0; p < plies; p++)
            {
                var legal = MoveGenerator.GenerateLegalMoves(state, state.CurrentTurn);
                if (legal.Count == 0 || state.Status != GameStatus.InProgress)
                {
                    live = false;
                    break;
                }
                state = GameController.ApplyMove(state, legal[rng.Next(legal.Count)]);
            }
            if (!live)
                continue;

            for (int depth = 1; depth <= 4; depth++)
                DumpSearch(writer, $"F{dumped}", state, depth);
            dumped++;
        }

        Console.WriteLine($"Dumped {entries.Count} suite positions (depths 1..8) and {dumped} fuzz positions (depths 1..4) to {outPath}.");
        return 0;
    }

    /// <summary>
    /// Perft oracle dump for the Rust spike's parity gate: fuzz positions ×
    /// depths 1..4, one line per (position, depth): count, 63-byte hex piece
    /// layout, side to move. Counts are order-independent, so the dump pins
    /// the movegen semantics the port must match.
    /// </summary>
    internal static int RunPerftDump(string[] args)
    {
        string outPath = Args.ReadString(args, "--perft-dump")
            ?? throw new InvalidOperationException("--perft-dump <file> is required.");
        int fuzzCount = Args.ReadInt(args, "--fuzz", 200);
        int fuzzSeed = Args.ReadInt(args, "--seed", 20240816);

        var positions = new List<GameState> { GameState.CreateInitial() };
        for (int f = 0; f < fuzzCount; f++)
        {
            var rng = new Random(fuzzSeed * 397 + f);
            int plies = 1 + rng.Next(60);
            var state = GameState.CreateInitial();
            bool live = true;
            for (int p = 0; p < plies; p++)
            {
                var legal = MoveGenerator.GenerateLegalMoves(state, state.CurrentTurn);
                if (legal.Count == 0 || state.Status != GameStatus.InProgress)
                {
                    live = false;
                    break;
                }
                state = GameController.ApplyMove(state, legal[rng.Next(legal.Count)]);
            }
            if (live)
                positions.Add(state);
        }

        using var writer = new StreamWriter(outPath);
        for (int i = 0; i < positions.Count; i++)
        {
            var board = SearchBoard.FromGameState(positions[i]);
            int side = (int)positions[i].CurrentTurn;
            for (int depth = 1; depth <= 4; depth++)
            {
                ulong count = PerftCount(board, side, depth);
                writer.WriteLine($"P{i} D{depth} {count} {HexLayout(positions[i])} {side}");
            }
        }
        Console.WriteLine($"Perft dump: {positions.Count} positions × depths 1..4 → {outPath}.");
        return 0;
    }

    private static ulong PerftCount(SearchBoard board, int side, int depth)
    {
        if (depth == 0)
            return 1;
        Span<SearchMove> moves = stackalloc SearchMove[SearchBoard.MaxMovesPerPly];
        int n = board.GenerateMoves(side, moves);
        if (depth == 1)
            return (ulong)n;
        ulong total = 0;
        for (int i = 0; i < n; i++)
        {
            var clone = board.Clone();
            clone.ApplyMove(moves[i]);
            total += PerftCount(clone, side ^ 1, depth - 1);
        }
        return total;
    }

    /// <summary>63 bytes square→pieceId as hex pairs (00 = empty).</summary>
    private static string HexLayout(GameState state)
    {
        var sb = new System.Text.StringBuilder(126);
        Span<byte> grid = stackalloc byte[63];
        foreach (var kv in state.Pieces)
        {
            int sq = kv.Key.Row * 7 + kv.Key.Col;
            grid[sq] = (byte)(((int)kv.Value.Animal - 1) * 2 + (int)kv.Value.Owner + 1);
        }
        foreach (byte b in grid)
            sb.Append(b.ToString("x2", System.Globalization.CultureInfo.InvariantCulture));
        return sb.ToString();
    }

    /// <summary>
    /// Dumps the piece-id assignment order and the move-generation order for the
    /// start position + seeded fuzz positions — the oracle for the Rust port's
    /// move ordering (ImmutableDictionary iteration order drives id assignment,
    /// which drives move generation order, which drives every node count).
    /// </summary>
    internal static int RunOrderDump(string[] args)
    {
        string outPath = Args.ReadString(args, "--order-dump")
            ?? throw new InvalidOperationException("--order-dump <file> is required.");
        int fuzz = Args.ReadInt(args, "--fuzz", 8);
        int fuzzSeed = Args.ReadInt(args, "--seed", 20240816);

        var positions = new List<GameState> { GameState.CreateInitial() };
        for (int f = 0; f < fuzz; f++)
        {
            var rng = new Random(fuzzSeed * 397 + f);
            int plies = 1 + rng.Next(40);
            var s = GameState.CreateInitial();
            for (int p = 0; p < plies && s.Status == GameStatus.InProgress; p++)
            {
                var legal = MoveGenerator.GenerateLegalMoves(s, s.CurrentTurn);
                if (legal.Count == 0) break;
                s = GameController.ApplyMove(s, legal[rng.Next(legal.Count)]);
            }
            positions.Add(s);
        }

        using var writer = new StreamWriter(outPath);
        var movesBuf = new SearchMove[SearchBoard.MaxMovesPerPly];
        for (int i = 0; i < positions.Count; i++)
        {
            var board = SearchBoard.FromGameState(positions[i]);
            int side = (int)positions[i].CurrentTurn;

            // Move-generation order, preceded by the hex layout so the Rust side
            // can reconstruct the exact position.
            int n = board.GenerateMoves(side, movesBuf);
            var moveTexts = new List<string>();
            for (int m = 0; m < n; m++)
                moveTexts.Add($"{movesBuf[m].From}>{movesBuf[m].To}");
            writer.WriteLine($"P{i} hex={HexLayout(positions[i])} stm={side} moves={string.Join(",", moveTexts)}");
        }
        Console.WriteLine($"Order dump → {outPath}.");
        return 0;
    }

    /// <summary>
    /// Eval oracle for the Rust port: dumps the static evaluation (both sides,
    /// all legacyEval x legacyP5 combos) for the start position + seeded fuzz.
    /// </summary>
    internal static int RunEvalDump(string[] args)
    {
        string outPath = Args.ReadString(args, "--eval-dump")
            ?? throw new InvalidOperationException("--eval-dump <file> is required.");
        int fuzz = Args.ReadInt(args, "--fuzz", 50);
        int fuzzSeed = Args.ReadInt(args, "--seed", 20240816);

        var positions = new List<GameState> { GameState.CreateInitial() };
        for (int f = 0; f < fuzz; f++)
        {
            var rng = new Random(fuzzSeed * 397 + f);
            int plies = 1 + rng.Next(60);
            var s = GameState.CreateInitial();
            for (int p = 0; p < plies && s.Status == GameStatus.InProgress; p++)
            {
                var legal = MoveGenerator.GenerateLegalMoves(s, s.CurrentTurn);
                if (legal.Count == 0) break;
                s = GameController.ApplyMove(s, legal[rng.Next(legal.Count)]);
            }
            positions.Add(s);
        }

        using var writer = new StreamWriter(outPath);
        for (int i = 0; i < positions.Count; i++)
        {
            var board = SearchBoard.FromGameState(positions[i]);
            int side = (int)positions[i].CurrentTurn;
            var vals = new List<int>();
            for (int s = 0; s < 2; s++)
                foreach (bool legacyEval in new[] { false, true })
                    foreach (bool legacyP5 in new[] { false, true })
                        vals.Add(EvaluationFunction.EvaluateStatic(board, s, legacyEval, legacyP5));
            // Also dump the full eval (with mobility) for the default config.
            int m0 = board.CountLegalMoves(0);
            int m1 = board.CountLegalMoves(1);
            writer.WriteLine($"P{i} hex={HexLayout(positions[i])} stm={side} eval={string.Join(",", vals)} mob={m0},{m1} full={EvaluationFunction.Evaluate(board, 0, m0, m1)},{EvaluationFunction.Evaluate(board, 1, m1, m0)}");
        }
        Console.WriteLine($"Eval dump → {outPath}.");
        return 0;
    }

    /// <summary>
    /// Full-search oracle for the Rust port: fixed-depth searches (pure search,
    /// useTablebase off) over the suite + seeded fuzz positions, dumping the hex
    /// layout, the game history hashes, and the (node count, best move) so the
    /// Rust engine can reproduce and compare byte-for-byte.
    /// </summary>
    internal static int RunSearchDump(string[] args)
    {
        string outPath = Args.ReadString(args, "--search-dump")
            ?? throw new InvalidOperationException("--search-dump <file> is required.");
        int fuzz = Args.ReadInt(args, "--fuzz", 20);
        int fuzzSeed = Args.ReadInt(args, "--seed", 20240816);
        int suiteMaxDepth = Args.ReadInt(args, "--suite-depth", 8);
        int fuzzMaxDepth = Args.ReadInt(args, "--fuzz-depth", 4);
        bool legacyEval = args.Contains("--legacy-eval");
        bool legacySearch = args.Contains("--legacy-search");
        bool legacyP5 = args.Contains("--legacy-p5");
        bool legacyP6 = args.Contains("--legacy-p6");
        bool useTb = args.Contains("--use-tb");

        var positions = new List<GameState>();
        foreach (var entry in TestSuiteParser.ParseFile(
            Args.ResolveDataPath(Args.ReadString(args, "--positions", DefaultPositionsPath) ?? DefaultPositionsPath)))
            positions.Add(entry.State);

        for (int f = 0; f < fuzz; f++)
        {
            var rng = new Random(fuzzSeed * 397 + f);
            int plies = 1 + rng.Next(60);
            var s = GameState.CreateInitial();
            for (int p = 0; p < plies && s.Status == GameStatus.InProgress; p++)
            {
                var legal = MoveGenerator.GenerateLegalMoves(s, s.CurrentTurn);
                if (legal.Count == 0) break;
                s = GameController.ApplyMove(s, legal[rng.Next(legal.Count)]);
            }
            if (s.Status == GameStatus.InProgress)
                positions.Add(s);
        }

        using var writer = new StreamWriter(outPath);
        for (int i = 0; i < positions.Count; i++)
        {
            int maxD = i < positions.Count - fuzz ? suiteMaxDepth : fuzzMaxDepth;
            for (int depth = 1; depth <= maxD; depth++)
            {
                var engine = new MinimaxEngine(TimeSpan.FromSeconds(60), depth,
                    legacyEval: legacyEval, legacySearch: legacySearch, legacyP5: legacyP5, legacyP6: legacyP6, useTablebase: useTb);
                var move = engine.FindBestMove(positions[i]);
                string moveText = move == null ? "null" : $"{move.Value.From.Row * 7 + move.Value.From.Col}>{move.Value.To.Row * 7 + move.Value.To.Col}";
                writer.WriteLine($"P{i} hex={HexLayout(positions[i])} hist={HistoryHex(positions[i])} stm={(int)positions[i].CurrentTurn} D{depth} {engine.NodesSearched} {moveText}");
            }
        }
        Console.WriteLine($"Search dump → {outPath}.");
        return 0;
    }

    private static string HistoryHex(GameState state) =>
        string.Join(",", state.History.Select(h => h.ToString("x16", System.Globalization.CultureInfo.InvariantCulture)));

    /// <summary>
    /// Tablebase probe oracle for the Rust port: dumps (hex, stm, wdl) for
    /// seeded random <= 3-piece positions, so the Rust probe can be verified
    /// byte-for-byte against the C# TablebaseProbe.
    /// </summary>
    internal static int RunTablebaseDump(string[] args)
    {
        string outPath = Args.ReadString(args, "--tb-dump")
            ?? throw new InvalidOperationException("--tb-dump <file> is required.");
        int samples = Args.ReadInt(args, "--samples", 2000);
        int seed = Args.ReadInt(args, "--seed", 42);
        string tbPath = Args.ReadString(args, "--tb-path") ?? "";

        if (tbPath.Length > 0)
            TablebaseProbe.LoadForTestingFromPath(tbPath);
        else
            TablebaseProbe.Initialize();

        if (!TablebaseProbe.IsLoaded)
        {
            Console.Error.WriteLine("No tablebase loaded — set --tb-path or JUNGLE_TB_PATH.");
            return 1;
        }

        var rng = new Random(seed);
        var animals = new[] { Animal.Rat, Animal.Cat, Animal.Dog, Animal.Wolf, Animal.Leopard, Animal.Tiger, Animal.Lion, Animal.Elephant };
        using var writer = new StreamWriter(outPath);
        for (int i = 0; i < samples; i++)
        {
            int pieceCount = rng.Next(2) == 0 ? 2 : 3;
            int stm = rng.Next(2);
            var pieces = new List<Piece>();
            if (pieceCount == 2)
            {
                var s1 = RandomUsable(rng);
                var s2 = RandomUsable(rng);
                while (s2 == s1) s2 = RandomUsable(rng);
                pieces.Add(new Piece(animals[rng.Next(8)], stm == 0 ? Player.Blue : Player.Red, s1));
                pieces.Add(new Piece(animals[rng.Next(8)], stm == 0 ? Player.Red : Player.Blue, s2));
            }
            else
            {
                bool twoBlue = rng.Next(2) == 0;
                var s1 = RandomUsable(rng);
                var s2 = RandomUsable(rng);
                var s3 = RandomUsable(rng);
                while (s2 == s1) s2 = RandomUsable(rng);
                while (s3 == s1 || s3 == s2) s3 = RandomUsable(rng);
                if (twoBlue)
                {
                    pieces.Add(new Piece(animals[rng.Next(8)], Player.Blue, s1));
                    pieces.Add(new Piece(animals[rng.Next(8)], Player.Blue, s2));
                    pieces.Add(new Piece(animals[rng.Next(8)], Player.Red, s3));
                }
                else
                {
                    pieces.Add(new Piece(animals[rng.Next(8)], Player.Blue, s1));
                    pieces.Add(new Piece(animals[rng.Next(8)], Player.Red, s2));
                    pieces.Add(new Piece(animals[rng.Next(8)], Player.Red, s3));
                }
            }

            var state = GameState.CreateFromPieces(pieces, stm == 0 ? Player.Blue : Player.Red);
            var board = SearchBoard.FromGameState(state);
            bool found = TablebaseProbe.TryProbe(board, 0, out int score);
            int wdl = found ? (score > 0 ? 3 : score < 0 ? 1 : 2) : 0;
            writer.WriteLine($"hex={HexLayout(state)} stm={stm} wdl={wdl} score={score}");
        }
        Console.WriteLine($"Tablebase dump → {outPath}.");
        return 0;

        static Position RandomUsable(Random r)
        {
            Position p;
            do { p = new Position(r.Next(7), r.Next(9)); }
            while (Board.Initial.IsDen(p, Player.Blue) || Board.Initial.IsDen(p, Player.Red));
            return p;
        }
    }

    private static void DumpSearch(StreamWriter? writer, string label, GameState state, int depth)
    {
        // Fresh engine per search: no TT/history carryover. A generous wall-clock
        // budget never aborts (fixed-depth searches finish far earlier once the
        // engine is JIT-warm), so node counts are fully deterministic.
        var engine = new MinimaxEngine(TimeSpan.FromSeconds(60), depth);
        var move = engine.FindBestMove(state);
        string moveText = move == null
            ? "null"
            : $"{move.Value.From.Col},{move.Value.From.Row}-{move.Value.To.Col},{move.Value.To.Row}";
        writer?.WriteLine($"{label} D{depth} {engine.NodesSearched} {moveText}");
    }
}
