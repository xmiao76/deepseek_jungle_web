using JungleGame.Core.Model;
using JungleGame.Web.Services;
using Xunit;

namespace JungleGame.Web.Tests;

public class BoardCoordinatesTests
{
    [Fact]
    public void Unflipped_MirrorsRows_SoBlueRendersAtBottom()
    {
        // Desktop screen mapping (CellOrigin): unflipped display = (col, 8-row),
        // so logical row 0 (Blue's den row) renders at the bottom of the board.
        var position = new Position(1, 2);

        Assert.Equal(1, BoardCoordinates.DisplayCol(position, flipped: false));
        Assert.Equal(6, BoardCoordinates.DisplayRow(position, flipped: false));
        Assert.Equal(position, BoardCoordinates.FromDisplay(1, 6, flipped: false));
    }

    [Fact]
    public void Unflipped_BlueDen_RendersAtBottomDisplayRow()
    {
        Assert.Equal(8, BoardCoordinates.DisplayRow(new Position(3, 0), flipped: false));
        Assert.Equal(0, BoardCoordinates.DisplayRow(new Position(3, 8), flipped: false));
    }

    [Fact]
    public void Flipped_MatchesDesktopScreenMapping()
    {
        // Desktop flipped screen mapping: (6-c, r).
        var position = new Position(1, 2);

        Assert.Equal(5, BoardCoordinates.DisplayCol(position, flipped: true));
        Assert.Equal(2, BoardCoordinates.DisplayRow(position, flipped: true));
    }

    [Fact]
    public void RoundTrip_IsIdentity_InBothModes()
    {
        for (var col = 0; col < BoardCoordinates.Cols; col++)
        {
            for (var row = 0; row < BoardCoordinates.Rows; row++)
            {
                var position = new Position(col, row);
                var dc = BoardCoordinates.DisplayCol(position, flipped: true);
                var dr = BoardCoordinates.DisplayRow(position, flipped: true);

                Assert.Equal(position, BoardCoordinates.FromDisplay(dc, dr, flipped: true));
            }
        }
    }

    [Fact]
    public void Flip_SwapsOppositeCorners()
    {
        // Flipped display (0,0) shows logical (6,0); display (6,8) shows logical (0,8).
        var topLeft = BoardCoordinates.FromDisplay(0, 0, flipped: true);
        var bottomRight = BoardCoordinates.FromDisplay(6, 8, flipped: true);

        Assert.Equal(new Position(6, 0), topLeft);
        Assert.Equal(new Position(0, 8), bottomRight);
    }

    [Theory]
    [InlineData(0, 0, "A1")]
    [InlineData(3, 0, "D1")] // Blue's den
    [InlineData(6, 8, "G9")]
    [InlineData(1, 1, "B2")]
    public void SquareName_UsesAlgebraicStyle(int col, int row, string expected)
    {
        Assert.Equal(expected, BoardCoordinates.SquareName(new Position(col, row)));
    }
}
