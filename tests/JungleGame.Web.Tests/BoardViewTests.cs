using Bunit;
using JungleGame.Core.Model;
using JungleGame.Web.Components;
using JungleGame.Web.Services;
using Xunit;

namespace JungleGame.Web.Tests;

public class BoardViewTests : TestContext
{
    [Fact]
    public void Renders_63Squares_16Pieces_TerrainMarkers()
    {
        var view = RenderBoard(this, new GameService());

        Assert.Equal(63, view.FindAll(".square").Count);
        Assert.Equal(16, view.FindAll(".piece").Count);
        // 6 traps (3 per side) with the ⚠ glyph, 2 dens with the 🏠 glyph.
        Assert.Equal(6, view.FindAll(".trap-glyph").Count);
        Assert.Equal(2, view.FindAll(".den-glyph").Count);
    }

    [Fact]
    public void Squares_CarryLogicalCoordinates_Unflipped()
    {
        var view = RenderBoard(this, new GameService());

        // First cell in DOM order is display (0,0), which is logical (0,0) when unflipped.
        Assert.Equal("0,0", view.FindAll(".square")[0].GetAttribute("data-square"));
    }

    [Fact]
    public void Squares_CarryLogicalCoordinates_Flipped()
    {
        var service = new GameService();
        service.ToggleFlip();
        var view = RenderBoard(this, service);

        // With the board flipped, display (0,0) maps to logical (6,8).
        Assert.Equal("6,8", view.FindAll(".square")[0].GetAttribute("data-square"));
    }

    [Fact]
    public void ClickingASquare_RaisesCallbackWithLogicalPosition()
    {
        var clicked = new List<Position>();
        var view = RenderBoard(this, new GameService(), clicked.Add);

        view.Find("[data-square=\"1,1\"]").Click();

        Assert.Equal(new Position(1, 1), Assert.Single(clicked));
    }

    [Fact]
    public void ClickingASquare_Flipped_RaisesLogicalPosition()
    {
        var service = new GameService();
        service.ToggleFlip();
        var clicked = new List<Position>();
        var view = RenderBoard(this, service, clicked.Add);

        // Display cell (0,0) shows logical (6,8) when flipped.
        view.Find("[data-square=\"6,8\"]").Click();

        Assert.Equal(new Position(6, 8), Assert.Single(clicked));
    }

    [Fact]
    public void Pieces_CarryDataAttributes_ForAutomation()
    {
        var view = RenderBoard(this, new GameService());

        Assert.Single(view.FindAll("[data-piece=\"blue-cat\"]"));
        Assert.Single(view.FindAll("[data-piece=\"blue-tiger\"]"));
        Assert.Single(view.FindAll("[data-piece=\"red-elephant\"]"));
    }

    [Fact]
    public void SelectionAndTargets_RenderHighlights()
    {
        var service = new GameService();
        service.OnSquareClicked(new Position(1, 1)); // select the blue cat
        var view = RenderBoard(this, service);

        Assert.Single(view.FindAll(".highlight-selected"));
        Assert.Equal(4, view.FindAll(".highlight-target").Count);
    }

    private static IRenderedComponent<BoardView> RenderBoard(TestContext ctx, GameService service, Action<Position>? onSquareClicked = null)
    {
        var callback = onSquareClicked ?? (_ => { });
        return ctx.RenderComponent<BoardView>(parameters => parameters
            .Add(p => p.Service, service)
            .Add(p => p.SquareClicked, (Action<Position>)callback));
    }
}
