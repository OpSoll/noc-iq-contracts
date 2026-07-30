# SLA Calculation Mathematical Spec and Edge-Case Boundary Guide

## Mathematical Specification
The Service Level Agreement (SLA) calculation computes penalties or rewards based on the Mean Time To Repair (MTTR) of an incident, compared to a predefined threshold for its severity.

Let:
- `M` = MTTR (in minutes)
- `T` = Threshold (in minutes)
- `P` = Penalty per minute (for M > T)
- `R` = Base Reward (for M <= T)

If `M <= T`:
  Status: MET
  Amount: `+R` (Reward)

If `M > T`:
  Status: VIOLATED
  Amount: `-(M - T) * P` (Penalty)

## Edge-Case Boundary Guide

1. **M = T (Exact Boundary)**
   - The SLA is considered MET.
   - The user receives the full reward `R`.
   - Penalty calculation is NOT triggered since the violation time is 0.

2. **M = 0 (Instant Resolution)**
   - Naturally `M <= T`, so the SLA is MET.
   - Full reward `R` is given.

3. **Missing Configuration**
   - If a severity is not configured in the SLA configuration maps, any outage calculation against it must immediately abort with a `ConfigNotFound` error.

4. **Extreme Penalty (M >> T)**
   - The penalty grows linearly. There is no built-in cap in this exact formula, so long outages may result in extremely large negative amounts.

5. **Paused Contract**
   - No SLA calculations can be performed when the contract is paused.
