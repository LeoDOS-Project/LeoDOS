While the Hungarian algorithm is based on **Set Logic** (find zeros, if no zeros, adjust sets), LAPJV is based on **Pathfinding Logic** (find the shortest path to a free node).

# Jonker-Volgenant (LAPJV) Algorithm

**Problem:** Same as Hungarian. Minimize total assignment cost.

**Example cost matrix:**
```
           N0    N1    N2
T0 [       1     4     4 ]
T1 [       1     3     4 ]
T2 [       4     2     1 ]
```

## Key Insight: The "Shortest Path"

In the Hungarian algorithm, we only looked at links with **Zero** cost. If we got stuck, we stopped and did math to create new zeros.

In LAPJV, we don't stop. We view the matrix as a **Graph**.
*   **Nodes:** The Tasks and the Nodes.
*   **Edges:** The cost to connect them is the **Reduced Cost**.

If Task 1 cannot take its favorite node, it calculates:
1.  **Option A:** "How expensive is it to just take my 2nd favorite node?"
2.  **Option B:** "How expensive is it to kick Task 0 out of N0, and force Task 0 to move to *their* 2nd favorite node?"

It calculates the **Shortest Path** (minimum total added cost) to find an empty slot. This is effectively **Dijkstra's Algorithm** running inside the matrix.

## Step 1: Initialization (Node Prices)

We maintain **Node Prices** (`v`). We ignore row potentials (`u`) for now and handle them implicitly during the search.
`reduced_cost[i][j] = cost[i][j] - v[j]`

**Initialize v (Column Minimums):**
```
v[0] = 1  (Min of col 0)
v[1] = 2  (Min of col 1)
v[2] = 1  (Min of col 2)
```

## Step 2: Assign Task 0 (Standard)

We look at the distances (reduced costs) from **T0** to all nodes.

**T0 Row:** `[1, 4, 4]`
**Prices v:** `[1, 2, 1]`

**Reduced Costs (Distances):**
```
T0->N0: 1 - 1 = 0
T0->N1: 4 - 2 = 2
T0->N2: 4 - 1 = 3
```
**Shortest distance:** 0 (at N0).
**Is N0 free?** Yes.
**Assign T0 -> N0.**

## Step 3: Assign Task 1 (The Dijkstra Search)

We calculate distances from **T1** to all nodes.

**T1 Row:** `[1, 3, 4]`
**Prices v:** `[1, 2, 1]`

**Initial Reduced Costs:**
```
T1->N0: 1 - 1 = 0
T1->N1: 3 - 2 = 1
T1->N2: 4 - 1 = 3
```

**The Conflict:**
The shortest distance is **0** (at N0).
*   **Is N0 free?** NO. It is held by **T0**.
*   We cannot stop. We must compare two paths:

**Path A: Go Direct (to N1)**
*   Distance: **1** (calculated above).

**Path B: The "Steal" (Kick T0 to N1)**
*   Cost to reach N0: **0**
*   **PLUS:** Cost for T0 to move to *its* next best option.
    *   T0 is currently at N0 (Cost 0).
    *   T0 to N1 cost: **2** (calculated in Step 2).
    *   T0 to N2 cost: **3**.
*   Total cost of Path B (T1->N0 + T0->N1): $0 + 2 = \mathbf{2}$.

**Compare:**
*   Path A (T1->N1): Cost **1**
*   Path B (T1->N0->N1): Cost **2**

**Winner:** Path A.
**Action:** T1 takes N1.

*(Note: In the Hungarian explanation, we hit a wall and adjusted potentials. Here, we realized instantly that paying cost 1 for N1 is cheaper than the cost 2 required to displace T0).*

**Price Update:**
We update `v` to reflect that N1 was "further away" than N0. This prevents future conflicts.
`v[N1]` decreases or stays relative to the path taken. (The code logic `v + dist - min_dist` ensures non-negative invariants).

## Step 4: Assign Task 2

We calculate distances from **T2**.

**T2 Row:** `[4, 2, 1]`
**Prices v:** `[1, 2, 1]`

**Reduced Costs:**
```
T2->N0: 4 - 1 = 3
T2->N1: 2 - 2 = 0
T2->N2: 1 - 1 = 0
```

**Analysis:**
1.  **N1 is distance 0.**
    *   Taken by T1.
    *   If we kick T1, T1 moves to N0 (Cost 0) or N1 (Cost 1).
    *   Path Cost: T2->N1(0) + T1->N0(0) = **0**.
2.  **N2 is distance 0.**
    *   Free.
    *   Path Cost: **0**.

Both paths cost 0. The algorithm picks the free one (N2).

**Assign T2 -> N2.**

### Note: What happens if we "Steal"? (Updating Prices)

In the steps above, we took the direct path. But let's look at the math if we had chosen to **steal N0** (Path B).

**1. Calculate Distance to N0**
T1 tries to take N0 directly.
```
dist[N0] = cost[T1][N0] - v[N0] = 1 - 1 = 0
```

**2. Calculate Distance to N1**
Since T1 took N0, the previous owner (T0) is pushed to N1. We add that cost to the previous distance.
``, `v[N1] = 2`
```
dist[N1] = dist[N0] + (cost[T0][N1] - v[N1]) = 0 + (4 - 2) = 2
```

**3. Identify min_dist**
Since N1 is the free node we successfully filled, its distance is the total path cost.
```
min_dist = dist[N1] = 2
```

**4. Update Prices**
We must update the price of the node we stole (N0).
```
v'[N0] = v[N0] + dist[N0] - min_dist = 1 + 0 - 2 = -1
```

**Why lower the price?**
Recall that `reduced_cost = cost - v`.
If `v` drops to -1, the expression `- v` becomes `+ 1`.
This **increases the reduced cost** for anyone else trying to use N0 in the future.

By lowering the price (`v`), the algorithm effectively puts a "High Traffic" surcharge on N0, discouraging T2 from trying to steal it again in the next step.

# Why is this Optimal?

Similar to the Hungarian algorithm, LAPJV relies on the concept of **Reduced Costs** and **Potentials**.

1.  **The Invariant:** The algorithm ensures that `reduced_cost` is always $\ge 0$.
2.  **The Shortest Path:** By using Dijkstra's algorithm, we guarantee that every time we assign a task, we are finding the path that increases the total cost of the system by the **minimum possible amount** allowed by the constraints.
3.  **The Result:** When all tasks are assigned, the sum of the potentials plus the sum of the path distances equals the Total Cost. Since we minimized the path distances at every single step, the final Total Cost is minimized.

## Hungarian vs. LAPJV

*   **Hungarian:** "Find zeros. If stuck, change the *entire matrix* math to create a new zero." (Global updates).
*   **LAPJV:** "Find the cheapest path, even if it's not zero. Only update the prices of the nodes we touched along that path." (Local updates).

In practice, LAPJV is often faster because it updates fewer numbers in memory when resolving conflicts.
