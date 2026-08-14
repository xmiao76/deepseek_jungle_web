using JungleGame.Core.Model;
using JungleGame.Web.Services;
using Xunit;

namespace JungleGame.Web.Tests;

public class BoardCoordinatesTests
{
    [Fact]
    public void Unflipped_MapsIdentity()
    {
        var position = new Position(3, 4);

        Assert.Equal(3, BoardCoordinates.DisplayCol(position, flipped: false));
        Assert.Equal(4, BoardCoordinates.DisplayRow(position, flipped: false));
        Assert.Equal(position, BoardCoordinates.FromDisplay(3, 4, flipped: false));
    }

    [Fact]
    public void Flipped_MirrorsBothAxes_AsDesktopGetVisualPosition()
    {
        var position = new Position(1, 2);

        // Desktop GetVisualPosition: (6-c, 8-r).
        Assert.Equal(5, BoardCoordinates.DisplayCol(position, flipped: true));
        Assert.Equal(6, BoardCoordinates.DisplayRow(position, flipped: true));
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
        var topLeft = BoardCoordinates.FromDisplay(0, 0, flipped: true);
        var bottomRight = BoardCoordinates.FromDisplay(6, 8, flipped: true);

        Assert.Equal(new Position(6, 8), topLeft);
        Assert.Equal(new Position(0, 0), bottomRight);
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
