using JungleGame.Core.Engine;
using JungleGame.Core.Model;
using Microsoft.JSInterop;

namespace JungleGame.Web.Services;

/// <summary>
/// The Rust engine backend: runs the search in a Web Worker (rust-engine.wasm)
/// via JS interop, so the CPU-bound search never blocks the UI thread. The C#
/// <see cref="MinimaxEngine"/> remains the default backend and the oracle; this
/// backend is selected with <c>?engine=rust</c>.
///
/// The search is time-bounded (iterative deepening to a wall-clock budget from
/// <c>performance.now()</c>, imported into the worker). Only fully completed
/// depths are accepted, mirroring the C# engine's time semantics.
/// </summary>
public sealed class RustEngineBackend
{
    private readonly IJSRuntime _js;
    private int _budgetMs;
    private int _lastDepth;
    private long _lastNodes;

    public RustEngineBackend(IJSRuntime js, TimeSpan timeLimit)
    {
        _js = js;
        _budgetMs = (int)timeLimit.TotalMilliseconds;
    }

    public int LastCompletedDepth => _lastDepth;
    public long NodesSearched => _lastNodes;

    public void SetTimeLimit(TimeSpan timeLimit) => _budgetMs = (int)timeLimit.TotalMilliseconds;

    /// <summary>Terminates the worker, abandoning any in-flight search.</summary>
    public void Reset()
    {
        // Fire-and-forget: reset is best-effort (a stale search is abandoned by
        // the page's epoch guard regardless).
        _ = _js.InvokeVoidAsync("rustEngine.reset").AsTask();
    }

    /// <summary>
    /// Searches for the best move. The position + game history (for three-fold
    /// repetition) are serialized to hex and handed to the worker; the result is
    /// decoded into a public <see cref="Move"/>.
    /// </summary>
    public async Task<Move?> FindBestMoveAsync(GameState state, CancellationToken token)
    {
        if (state.Status != GameStatus.InProgress)
            return null;

        string hex = SerializePieces(state);
        string historyHex = SerializeHistory(state);
        int turn = (int)state.CurrentTurn;
        const int maxDepth = 32;

        var result = await _js.InvokeAsync<RustSearchResult>(
            "rustEngine.search", hex, turn, historyHex, _budgetMs, maxDepth);

        _lastDepth = result.Depth;
        _lastNodes = result.Nodes;
        if (result.NoMove)
            return null;

        var from = new Position(result.From % 7, result.From / 7);
        var to = new Position(result.To % 7, result.To / 7);
        var captured = state.GetPieceAt(to);
        return new Move(from, to, captured);
    }

    /// <summary>Serializes the position into the 126-hex-char square->type layout.</summary>
    private static string SerializePieces(GameState state)
    {
        Span<char> hex = stackalloc char[126];
        for (int sq = 0; sq < 63; sq++)
        {
            hex[sq * 2] = '0';
            hex[sq * 2 + 1] = '0';
        }
        foreach (var kv in state.Pieces)
        {
            int sq = kv.Key.Row * 7 + kv.Key.Col;
            int type = ((int)kv.Value.Animal - 1) * 2 + (int)kv.Value.Owner;
            int id = type + 1;
            hex[sq * 2] = ToHex(id >> 4);
            hex[sq * 2 + 1] = ToHex(id & 0xF);
        }
        return new string(hex);
    }

    /// <summary>Serializes the game history as concatenated 16-hex-char u64 hashes.</summary>
    private static string SerializeHistory(GameState state)
    {
        if (state.History.IsEmpty)
            return string.Empty;
        var sb = new System.Text.StringBuilder(state.History.Count * 16);
        foreach (ulong hash in state.History)
            sb.Append(hash.ToString("x16", System.Globalization.CultureInfo.InvariantCulture));
        return sb.ToString();
    }

    private static char ToHex(int nibble) => (char)(nibble < 10 ? '0' + nibble : 'a' + nibble - 10);

    private sealed class RustSearchResult
    {
        public int Nodes { get; set; }
        public int Depth { get; set; }
        public int From { get; set; }
        public int To { get; set; }
        public bool NoMove { get; set; }
    }
}
