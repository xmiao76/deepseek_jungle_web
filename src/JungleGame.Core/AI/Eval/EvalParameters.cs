namespace JungleGame.Core.AI;

/// <summary>
/// Evaluation term weights — the tunable vector for the data-driven tuning
/// harness. Features are the side-relative integer counts extracted by
/// <see cref="EvalFeatureExtractor"/>; the score is the dot product. The
/// instance fields start at the hand-tuned constants the evaluation has
/// always carried; the tuning harness fits a new vector and adopts it via
/// <see cref="EvaluationFunction.SetParameters"/>. The legacyEval/legacyP5
/// variants suppress the P3/P5 features in the extractor, so the same weights
/// apply.
/// </summary>
internal sealed class EvalParameters
{
    // Defaults: the historical hand-tuned constants.
    internal int MaterialWeight = 100;                 // per rank
    internal int ForwardWeight = 1;                    // per progress × aggression point
    internal int DenOffenseWeight = 12;                // per (4 - distToOppDen), dist ≤ 3
    internal int DenGuardWeight = 8;                   // per (3 - distToOwnDen), dist ≤ 2
    internal int TrapPenalty = 80;                     // per piece on an enemy trap
    internal int DoomedPieceWeightPerRank = 5;         // per trapped piece rank (40 × rank/8)
    internal int DenEscortBonus = 30;                  // per rank ≥ 7 piece guarding a threatened den
    internal int RiverBankBonus = 15;                  // per Lion/Tiger on a bank square
    internal int JumpPathBonus = 10;                   // per Lion/Tiger with a jump toward the opp den
    internal int RatNearWaterBonus = 8;                // per Rat adjacent to water
    internal int RatInWaterBonus = 12;                 // per Rat in the water
    internal int ElephantRatFearWeight = 15;           // per (4 - dist) to enemy Rat, dist ≤ 3
    internal int ThreatStrongerWeight = 15;            // per rank threatened by a strictly stronger piece
    internal int ThreatEqualWeight = 8;                // per rank threatened by an equal piece
    internal int RatThreatensElephantPenalty = 25;     // per Elephant adjacent to enemy Rat on land
    internal int MobilityWeight = 3;                   // per extra legal move over the opponent
    internal int DenThreatWeight = 40;                 // per excess attacker near own den
    internal int EndgameDenThreatPenalty = 200;        // endgame: attackers with zero defenders
    internal int EndgameAdvance1Weight = 50;           // PSQT-lite: per Leopard+ at Manhattan 1 from the opp den (endgame)
    internal int EndgameAdvance2Weight = 25;           // PSQT-lite: per Leopard+ at Manhattan 2 from the opp den (endgame)
    internal int BackRankPenalty = 5;                  // per undeveloped piece on the home row
    internal int RatDenInfluenceWeight = 15;           // P5: per (3 - dist) for Rats near the opp den
    internal int FrontTrapGuardWeight = 20;            // P5: per own piece on the front trap
    internal int ClearJumpLaneWeight = 12;             // P5: per clear Lion/Tiger jump onto the enemy half

    internal static EvalParameters Default { get; } = new();

    internal const int WeightCount = 24; // matches ToVector/FromVector

    internal EvalParameters Clone() => (EvalParameters)MemberwiseClone();

    /// <summary>Serializes the weights into a double vector (tuning harness).</summary>
    internal void ToVector(double[] v, int offset)
    {
        v[offset + 0] = MaterialWeight;
        v[offset + 1] = ForwardWeight;
        v[offset + 2] = DenOffenseWeight;
        v[offset + 3] = DenGuardWeight;
        v[offset + 4] = TrapPenalty;
        v[offset + 5] = DoomedPieceWeightPerRank;
        v[offset + 6] = DenEscortBonus;
        v[offset + 7] = RiverBankBonus;
        v[offset + 8] = JumpPathBonus;
        v[offset + 9] = RatNearWaterBonus;
        v[offset + 10] = RatInWaterBonus;
        v[offset + 11] = ElephantRatFearWeight;
        v[offset + 12] = ThreatStrongerWeight;
        v[offset + 13] = ThreatEqualWeight;
        v[offset + 14] = RatThreatensElephantPenalty;
        v[offset + 15] = MobilityWeight;
        v[offset + 16] = DenThreatWeight;
        v[offset + 17] = EndgameDenThreatPenalty;
        v[offset + 18] = EndgameAdvance1Weight;
        v[offset + 19] = EndgameAdvance2Weight;
        v[offset + 20] = BackRankPenalty;
        v[offset + 21] = RatDenInfluenceWeight;
        v[offset + 22] = FrontTrapGuardWeight;
        v[offset + 23] = ClearJumpLaneWeight;
    }

    /// <summary>Adopts a tuned double vector back into the weight fields.</summary>
    internal void FromVector(double[] v, int offset)
    {
        MaterialWeight = (int)Math.Round(v[offset + 0]);
        ForwardWeight = (int)Math.Round(v[offset + 1]);
        DenOffenseWeight = (int)Math.Round(v[offset + 2]);
        DenGuardWeight = (int)Math.Round(v[offset + 3]);
        TrapPenalty = (int)Math.Round(v[offset + 4]);
        DoomedPieceWeightPerRank = (int)Math.Round(v[offset + 5]);
        DenEscortBonus = (int)Math.Round(v[offset + 6]);
        RiverBankBonus = (int)Math.Round(v[offset + 7]);
        JumpPathBonus = (int)Math.Round(v[offset + 8]);
        RatNearWaterBonus = (int)Math.Round(v[offset + 9]);
        RatInWaterBonus = (int)Math.Round(v[offset + 10]);
        ElephantRatFearWeight = (int)Math.Round(v[offset + 11]);
        ThreatStrongerWeight = (int)Math.Round(v[offset + 12]);
        ThreatEqualWeight = (int)Math.Round(v[offset + 13]);
        RatThreatensElephantPenalty = (int)Math.Round(v[offset + 14]);
        MobilityWeight = (int)Math.Round(v[offset + 15]);
        DenThreatWeight = (int)Math.Round(v[offset + 16]);
        EndgameDenThreatPenalty = (int)Math.Round(v[offset + 17]);
        EndgameAdvance1Weight = (int)Math.Round(v[offset + 18]);
        EndgameAdvance2Weight = (int)Math.Round(v[offset + 19]);
        BackRankPenalty = (int)Math.Round(v[offset + 20]);
        RatDenInfluenceWeight = (int)Math.Round(v[offset + 21]);
        FrontTrapGuardWeight = (int)Math.Round(v[offset + 22]);
        ClearJumpLaneWeight = (int)Math.Round(v[offset + 23]);
    }

    internal int Dot(in EvalFeatures f)
    {
        int score =
            (f.MaterialMy - f.MaterialOpp) * MaterialWeight +
            (f.ForwardMy - f.ForwardOpp) * ForwardWeight +
            (f.DenOffenseMy - f.DenOffenseOpp) * DenOffenseWeight +
            (f.DenGuardMy - f.DenGuardOpp) * DenGuardWeight +
            (-f.TrapMy + f.TrapOpp) * TrapPenalty +
            (-f.DoomedRankSumMy + f.DoomedRankSumOpp) * DoomedPieceWeightPerRank +
            (f.EscortMy - f.EscortOpp) * DenEscortBonus +
            (f.RiverBankMy - f.RiverBankOpp) * RiverBankBonus +
            (f.JumpPathMy - f.JumpPathOpp) * JumpPathBonus +
            (f.RatNearWaterMy - f.RatNearWaterOpp) * RatNearWaterBonus +
            (f.RatInWaterMy - f.RatInWaterOpp) * RatInWaterBonus +
            (-f.ElephantFearMy + f.ElephantFearOpp) * ElephantRatFearWeight +
            (-f.StrongerThreatSumMy + f.StrongerThreatSumOpp) * ThreatStrongerWeight +
            (-f.EqualThreatSumMy + f.EqualThreatSumOpp) * ThreatEqualWeight +
            (-f.RatElephantMy + f.RatElephantOpp) * RatThreatensElephantPenalty +
            -f.DenThreatExcess * DenThreatWeight -
            f.EndgameDenThreat * EndgameDenThreatPenalty +
            (f.EndgameAdvance1My - f.EndgameAdvance1Opp) * EndgameAdvance1Weight +
            (f.EndgameAdvance2My - f.EndgameAdvance2Opp) * EndgameAdvance2Weight +
            (-f.BackRankMy + f.BackRankOpp) * BackRankPenalty +
            (f.RatDenInfluenceMy - f.RatDenInfluenceOpp) * RatDenInfluenceWeight +
            (f.FrontTrapGuardMy - f.FrontTrapGuardOpp) * FrontTrapGuardWeight +
            (f.ClearJumpLaneMy - f.ClearJumpLaneOpp) * ClearJumpLaneWeight;
        return score;
    }
}
