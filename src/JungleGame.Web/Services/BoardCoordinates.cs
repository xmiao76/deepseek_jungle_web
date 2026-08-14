using JungleGame.Core.Model;

namespace JungleGame.Web.Services;

/// <summary>
/// Pure mapping between board coordinates and display coordinates. Board flipping only
/// changes this mapping (visual); it never touches game state.
///
/// The mapping reproduces the desktop app's on-screen geometry (MainWindow.xaml.cs
/// CellOrigin paints visual row 0 at the canvas bottom, so logical rows 0-2 — Blue —
/// render at the bottom edge of the board): unflipped display = (col, 8-row);
/// flipped display = (6-col, row) — a 180° rotation of the unflipped view.
/// </summary>
public static class BoardCoordinates
{
    public const int Cols = 7;
    public const int Rows = 9;

    public static int DisplayCol(Position p, bool flipped) =>
        flipped ? Cols - 1 - p.Col : p.Col;

    public static int DisplayRow(Position p, bool flipped) =>
        flipped ? p.Row : Rows - 1 - p.Row;

    public static Position FromDisplay(int displayCol, int displayRow, bool flipped) =>
        new(
            flipped ? Cols - 1 - displayCol : displayCol,
            flipped ? displayRow : Rows - 1 - displayRow);

    /// <summary>Algebraic-style square name: columns A–G, rows 1–9 (e.g. Blue's den is D1).</summary>
    public static string SquareName(Position p) => $"{(char)('A' + p.Col)}{p.Row + 1}";
}
