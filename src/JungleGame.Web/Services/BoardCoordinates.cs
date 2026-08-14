using JungleGame.Core.Model;

namespace JungleGame.Web.Services;

/// <summary>
/// Pure mapping between board coordinates and display coordinates. Board flipping only
/// changes this mapping (visual, the desktop GetVisualPosition semantics); it never
/// touches game state.
/// </summary>
public static class BoardCoordinates
{
    public const int Cols = 7;
    public const int Rows = 9;

    public static int DisplayCol(Position p, bool flipped) =>
        flipped ? Cols - 1 - p.Col : p.Col;

    public static int DisplayRow(Position p, bool flipped) =>
        flipped ? Rows - 1 - p.Row : p.Row;

    public static Position FromDisplay(int displayCol, int displayRow, bool flipped) =>
        new(
            flipped ? Cols - 1 - displayCol : displayCol,
            flipped ? Rows - 1 - displayRow : displayRow);

    /// <summary>Algebraic-style square name: columns A–G, rows 1–9 (e.g. Blue's den is D1).</summary>
    public static string SquareName(Position p) => $"{(char)('A' + p.Col)}{p.Row + 1}";
}
