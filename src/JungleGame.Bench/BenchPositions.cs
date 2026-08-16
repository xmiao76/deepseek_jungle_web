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
