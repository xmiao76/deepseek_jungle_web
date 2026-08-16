using JungleGame.Core.AI;

namespace JungleGame.Tests.Helpers;

/// <summary>
/// VERBATIM copy of the pre-linearization EvaluationFunction.EvaluateStatic
/// implementation (the hand-rolled loops, P3+P5 era). The linearized
/// EvaluationFunction must produce byte-identical scores to this reference —
/// the differential fuzz test over seeded positions is the score-identity
/// gate. Do not "improve" this file: it exists only as the historical oracle.
/// </summary>
internal static class StaticEvalReference
{
    // Material values in centipawns
    private static readonly int[] MaterialValue = { 0, 100, 200, 300, 400, 500, 600, 700, 800 };
    // Indexed by (int)Animal: Rat=1..Elephant=8

    // Term weights, named so A/B tuning is reviewable in one place
    private const int DenOffenseWeight = 12;
    private const int DenGuardWeight = 8;
    private const int TrapPenalty = 80;
    private const int DoomedPieceWeight = 40;
    private const int DenEscortBonus = 30;
    private const int RiverBankBonus = 15;
    private const int JumpPathBonus = 10;
    private const int RatNearWaterBonus = 8;
    private const int RatInWaterBonus = 12;
    private const int ElephantRatFearWeight = 15;
    private const int ThreatStrongerWeight = 15;
    private const int ThreatEqualWeight = 8;
    private const int RatThreatensElephantPenalty = 25;
    private const int DenThreatWeight = 40;
    private const int EndgameDenThreatPenalty = 200;
    private const int EndgameAdvanceWeight = 25;
    private const int BackRankPenalty = 5;
    private const int EndgamePieceCount = 8;
    private const int DevelopmentMaterialGate = 20;
    private const int RatDenInfluenceWeight = 15;
    private const int FrontTrapGuardWeight = 20;
    private const int ClearJumpLaneBonus = 12;

    private const byte WaterTerrain = 1;

    private static int ForwardBonus(int rank, int row, int owner)
    {
        int progress = owner == 0 ? row : (8 - row);

        int aggression = rank switch
        {
            7 => 6,
            6 => 4,
            5 => 4,
            4 => 3,
            3 => 3,
            2 => 2,
            1 => 2,
            8 => 1,
            _ => 2
        };

        return progress * aggression;
    }

    private static int DenProximity(int sq, int owner)
    {
        int col = sq % 7;
        int row = sq / 7;
        int ownDenCol = 3, ownDenRow = owner == 0 ? 0 : 8;
        int oppDenCol = 3, oppDenRow = owner == 0 ? 8 : 0;

        int distToOwnDen = Math.Abs(col - ownDenCol) + Math.Abs(row - ownDenRow);
        int distToOppDen = Math.Abs(col - oppDenCol) + Math.Abs(row - oppDenRow);

        int score = 0;

        if (distToOppDen <= 3)
            score += (4 - distToOppDen) * DenOffenseWeight;

        if (distToOwnDen <= 2)
            score += (3 - distToOwnDen) * DenGuardWeight;

        return score;
    }

    private static int ManhattanDistance(int sqA, int sqB) =>
        Math.Abs(sqA % 7 - sqB % 7) + Math.Abs(sqA / 7 - sqB / 7);

    internal static int EvaluateStatic(SearchBoard board, int side, bool legacyEval = false, bool legacyP5 = false)
    {
        int score = 0;

        int totalPieces = board.PieceCount(0) + board.PieceCount(1);
        bool isEndgame = totalPieces <= EndgamePieceCount;

        bool blueDenThreatened = false;
        bool redDenThreatened = false;
        if (!legacyEval)
        {
            foreach (byte enemy in board.PieceIds(1))
            {
                if (ManhattanDistance(board.SquareOf(enemy), 3) <= 3)
                {
                    blueDenThreatened = true;
                    break;
                }
            }
            foreach (byte enemy in board.PieceIds(0))
            {
                if (ManhattanDistance(board.SquareOf(enemy), 59) <= 3)
                {
                    redDenThreatened = true;
                    break;
                }
            }
        }

        for (int s = 0; s < 2; s++)
        {
            foreach (byte id in board.PieceIds(s))
            {
                int sq = board.SquareOf(id);
                bool ownDenThreatened = s == 0 ? blueDenThreatened : redDenThreatened;
                int pieceScore = EvaluatePiece(board, id, sq, isEndgame, legacyEval, legacyP5, ownDenThreatened);
                score += s == side ? pieceScore : -pieceScore;
            }
        }

        int ownDenCol = 3, ownDenRow = side == 0 ? 0 : 8;
        int defendersNearDen = 0;
        int attackersNearDen = 0;

        for (int s = 0; s < 2; s++)
        {
            foreach (byte id in board.PieceIds(s))
            {
                int sq = board.SquareOf(id);
                int col = sq % 7;
                int row = sq / 7;
                if (Math.Abs(col - ownDenCol) + Math.Abs(row - ownDenRow) <= 2)
                {
                    if (s == side)
                        defendersNearDen++;
                    else
                        attackersNearDen++;
                }
            }
        }

        if (attackersNearDen > defendersNearDen)
            score -= DenThreatWeight * (attackersNearDen - defendersNearDen);
        if (isEndgame && attackersNearDen > 0 && defendersNearDen == 0)
            score -= EndgameDenThreatPenalty;

        return score;
    }

    private static int EvaluatePiece(SearchBoard board, byte id, int sq, bool isEndgame, bool legacyEval, bool legacyP5, bool ownDenThreatened)
    {
        int rank = SearchBoard.RankOf[id];
        int owner = ((id - 1) % SearchBoard.DistinctPieceKinds) & 1;
        int row = sq / 7;
        int col = sq % 7;

        int score = MaterialValue[rank];

        score += ForwardBonus(rank, row, owner);
        score += DenProximity(sq, owner);

        if (!legacyP5 && sq == (owner == 0 ? 10 : 52))
            score += FrontTrapGuardWeight;

        if (SearchBoard.EffectiveRankOf(id, sq) == 0)
        {
            score -= TrapPenalty;
            if (!legacyEval)
                score -= DoomedPieceWeight * rank / 8;
        }

        if (!legacyEval && rank >= 7 && ownDenThreatened)
        {
            int ownDenCol = 3, ownDenRow = owner == 0 ? 0 : 8;
            int distToOwnDen = Math.Abs(col - ownDenCol) + Math.Abs(row - ownDenRow);
            if (distToOwnDen <= 2)
                score += DenEscortBonus;
        }

        if (rank == 7 || rank == 6)
        {
            if (IsRiverBank(sq))
                score += RiverBankBonus;

            int oppDenSq = owner == 0 ? 59 : 3;
            if (HasRiverBetween(sq, oppDenSq))
                score += JumpPathBonus;

            if (!legacyP5 && SearchBoard.HasClearJumpToEnemyHalf(board, id, sq))
                score += ClearJumpLaneBonus;
        }

        if (rank == 1)
        {
            if (IsAdjacentToWater(sq))
                score += RatNearWaterBonus;
            if (SearchBoard.TerrainOf[sq] == WaterTerrain)
                score += RatInWaterBonus;

            if (!legacyP5)
            {
                int oppDenCol = 3, oppDenRow = owner == 0 ? 8 : 0;
                int distToOppDen = Math.Abs(col - oppDenCol) + Math.Abs(row - oppDenRow);
                if (distToOppDen <= 2)
                    score += (3 - distToOppDen) * RatDenInfluenceWeight;
            }
        }

        if (rank == 8)
        {
            int opp = owner ^ 1;
            foreach (byte enemy in board.PieceIds(opp))
            {
                int enemySq = board.SquareOf(enemy);
                if (SearchBoard.RankOf[enemy] == 1 && SearchBoard.TerrainOf[enemySq] != WaterTerrain)
                {
                    int dist = ManhattanDistance(sq, enemySq);
                    if (dist <= 3)
                        score -= (4 - dist) * ElephantRatFearWeight;
                }
            }
        }

        foreach (byte neighborSq in SearchBoard.Neighbors[sq])
        {
            byte enemy = board.Occupant(neighborSq);
            if (enemy == 0 || ((enemy - 1) & 1) == owner)
                continue;

            int ourEffRank = SearchBoard.EffectiveRankOf(id, sq);
            int enemyEffRank = SearchBoard.EffectiveRankOf(enemy, neighborSq);
            int enemyRank = SearchBoard.RankOf[enemy];

            if (rank == 8 && enemyRank == 1)
            {
                if (SearchBoard.TerrainOf[neighborSq] != WaterTerrain)
                    score -= RatThreatensElephantPenalty;
            }
            else if (enemyEffRank >= ourEffRank)
            {
                if (SearchBoard.CanCapture(enemy, neighborSq, id, sq))
                {
                    if (enemyEffRank > ourEffRank)
                        score -= rank * ThreatStrongerWeight;
                    else
                        score -= rank * ThreatEqualWeight;
                }
            }
        }

        if (isEndgame && rank >= 5)
        {
            int oppDenCol = 3, oppDenRow = owner == 0 ? 8 : 0;
            int distToOppDen = Math.Abs(col - oppDenCol) + Math.Abs(row - oppDenRow);
            if (distToOppDen <= 2)
                score += (3 - distToOppDen) * EndgameAdvanceWeight;
        }

        int homeRow = owner == 0 ? 0 : 8;
        if (row == homeRow && TotalMaterialOnBoard(board) > DevelopmentMaterialGate)
            score -= BackRankPenalty;

        return score;
    }

    private static int TotalMaterialOnBoard(SearchBoard board)
    {
        int total = 0;
        foreach (byte id in board.PieceIds(0))
            total += SearchBoard.RankOf[id];
        foreach (byte id in board.PieceIds(1))
            total += SearchBoard.RankOf[id];
        return total;
    }

    private static bool IsRiverBank(int sq)
    {
        if (SearchBoard.TerrainOf[sq] == WaterTerrain) return false;

        foreach (byte adj in SearchBoard.Neighbors[sq])
        {
            if (SearchBoard.TerrainOf[adj] == WaterTerrain)
                return true;
        }
        return false;
    }

    private static bool IsAdjacentToWater(int sq)
    {
        foreach (byte adj in SearchBoard.Neighbors[sq])
        {
            if (SearchBoard.TerrainOf[adj] == WaterTerrain)
                return true;
        }
        return false;
    }

    private static bool HasRiverBetween(int fromSq, int toSq)
    {
        int fc = fromSq % 7, fr = fromSq / 7;
        int tc = toSq % 7, tr = toSq / 7;

        int stepCol = Math.Sign(tc - fc);
        int stepRow = Math.Sign(tr - fr);

        int col = fc + stepCol;
        int row = fr + stepRow;

        bool foundWater = false;
        while (col != tc || row != tr)
        {
            if (col >= 0 && col <= 6 && row >= 0 && row <= 8 &&
                SearchBoard.TerrainOf[row * 7 + col] == WaterTerrain)
                foundWater = true;

            col += stepCol;
            row += stepRow;

            if (Math.Abs(col - fc) > Math.Abs(tc - fc) && Math.Abs(row - fr) > Math.Abs(tr - fr))
                break;
        }

        return foundWater;
    }
}
