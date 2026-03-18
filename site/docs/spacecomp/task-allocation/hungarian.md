# Linear Sum Assignment Problem

**Problem:** Given n tasks and n nodes, with cost[i][j] = cost to assign task i to node j, find the assignment that minimizes total cost. Each task goes to exactly one node, each node handles exactly one task.

**Example cost matrix (3 tasks, 3 nodes):**
```
           Node 0   Node 1   Node 2
Task 0  [    10       5       13   ]
Task 1  [     3       9       18   ]
Task 2  [    18       7        2   ]
```

**Optimal:** Task 0→Node 1 (cost 5), Task 1→Node 0 (cost 3), Task 2→Node 2 (cost 2) = **Total: 10**

---

# Hungarian Algorithm

## Key Insight

Subtracting a constant from any row or column doesn't change which assignment is optimal, because every valid assignment uses exactly one cell per row and one cell per column.

We transform the matrix so that:
1. All values >= 0
2. Every row has at least one 0
3. Every column has at least one 0

Then we search for an assignment using only zeros. If we find one, it's optimal (can't do better than total cost 0 in the reduced matrix).

## Step 1: Create Reduced Cost Matrix

We maintain potentials `u[i]` for each task and `v[j]` for each node. The reduced cost is:
```
reduced[i][j] = cost[i][j] - u[i] - v[j]
```

**Initialize u (row minimums):**
```
u[0] = min(10, 5, 13) = 5
u[1] = min(3, 9, 18) = 3
u[2] = min(18, 7, 2) = 2
```

**Compute cost - u:**
```
           N0    N1    N2
T0 [       5     0     8 ]   (subtracted 5)
T1 [       0     6    15 ]   (subtracted 3)
T2 [      16     5     0 ]   (subtracted 2)
```

**Initialize v (column minimums of the above):**
```
v[0] = min(5, 0, 16) = 0
v[1] = min(0, 6, 5) = 0
v[2] = min(8, 15, 0) = 0
```

**Final reduced cost matrix (cost - u - v):**
```
           N0    N1    N2
T0 [       5     0     8 ]
T1 [       0     6    15 ]
T2 [       16    5     0 ]
```

Every row has a 0. Every column has a 0.

## Step 2: Find Matching Through Zeros

Zeros in the reduced matrix:
```
           N0    N1    N2
T0 [       -     0     - ]    T0 can use: N1
T1 [       0     -     - ]    T1 can use: N0
T2 [       -     -     0 ]    T2 can use: N2
```

**Search:**
```
T0 -> N1 (only option)
T1 -> N0 (only option)
T2 -> N2 (only option)

Complete matching found!
```

No conflicts, no backtracking needed.

## Step 3: Result

```
T0 -> N1 (original cost: 5)
T1 -> N0 (original cost: 3)
T2 -> N2 (original cost: 2)
                ----------
            Total: 10
```

---

# Example Where No Matching Exists Through Zeros

**Original cost matrix:**
```
           N0    N1    N2
T0 [       1     4     4 ]
T1 [       1     3     4 ]
T2 [       4     2     1 ]
```

**Initialize u:**
```
u[0] = 1, u[1] = 1, u[2] = 1
```

**Cost - u:**
```
           N0    N1    N2
T0 [       0     3     3 ]
T1 [       0     2     3 ]
T2 [       3     1     0 ]
```

**Initialize v:**
```
v[0] = 0, v[1] = 1, v[2] = 0
```

**Reduced costs (cost - u - v):**
```
           N0    N1    N2
T0 [       0     2     3 ]
T1 [       0     1     3 ]
T2 [       3     0     0 ]
```

Zeros:
```
T0: N0
T1: N0
T2: N1, N2
```

## How the Algorithm Proceeds

We process tasks one at a time. If a task wants a node that is taken, we perform a **Recursive Check**: we ask the current owner if *they* can move to a different zero.

**Definitions:**
- **Visited Tasks:** Tasks we have asked to move.
- **Visited Nodes:** Nodes involved in the conflict.

---

**Match T0:**
```
Search for T0:
  Look at row T0. Zero at N0. Is N0 free? Yes.

Assign T0 -> N0.
```

**Match T1 (The Conflict):**
```
Search for T1:
  Look at row T1. Zero at N0.
  Is N0 free? NO. It is held by T0.

  Conflict! We must check if T0 can move.
  Mark Visited: Tasks {T1, T0}, Nodes {N0}

  Recursive Check (Ask T0):
    Look at row T0 options: [0, 2, 3]
    - Zero at N0? Yes, but we are already fighting over N0.
    - Zero at N1? No (Cost is 2).
    - Zero at N2? No (Cost is 3).

  Result: T0 has no other zero. T0 cannot move.

Search failed.
```

## Adjusting Potentials

We must create a new zero to break the deadlock. We calculate **Delta** by finding the cheapest "Escape Route" from **Visited Tasks** to **Unvisited Nodes**.

*   **Visited Tasks:** \{T0, T1\}
*   **Unvisited Nodes:** \{N1, N2\}

**Check the specific costs:**

```
           N1    N2
T0 [       2     3 ]
T1 [       1     3 ]
```

```
T0 -> N1: Cost 2
T0 -> N2: Cost 3
T1 -> N1: Cost 1   <-- Minimum (Delta)
T1 -> N2: Cost 3

delta = 1
```

**Adjust Potentials:**
```
u[visited tasks] += delta:   u[T0] += 1,  u[T1] += 1
v[visited nodes] -= delta:   v[N0] -= 1
```

New reduced cost matrix: `reduced[i][j] = cost[i][j] - u[i] - v[j]`

**New reduced matrix:**
```
           N0    N1    N2
T0 [       0     1     2 ]   (2 became 1)
T1 [       0     0     2 ]   (1 became 0 -> NEW ZERO!)
T2 [       3     0     0 ]
```

This reduces the cost in unvisited columns for the trapped tasks.

We then continue the search from where we left off.

**Search again (Retry T1):**
```
Attempt N0:
  Taken by T0. Recursive check: Can T0 move?
  T0 has no other zero. Path blocked.

Attempt N1:
  Found zero (created by the adjustment).
  Is N1 free? Yes.

Assign T1 -> N1
```

**Final Match (T2):**
```
Search for T2:
  Look at row T2. Zeros at N1 and N2.

  Attempt N1:
    Taken by T1. Recursive check: Can T1 move?
    T1 can move to N0, but N0 is taken by T0 (who cannot move).
    Path blocked.

  Attempt N2:
    Is N2 free? Yes.

Assign T2 -> N2
```

**Final Assignment:** T0->N0, T1->N1, T2->N2

Here is the text to add at the very end. It connects the "potentials" logic back to your original question about why valid equals optimal.

---

# Why is this Optimal?

You might wonder: "We found *a* solution using zeros, but how do we know it's the *best* solution?"

This is guaranteed by the **Potentials (`u` and `v`)**.

Recall our formula:
`reduced[i][j] = cost[i][j] - u[i] - v[j]`

We can rearrange this to see what the Original Cost is composed of:
`cost[i][j] = u[i] + v[j] + reduced[i][j]`

**The Lower Bound (The Floor)**
The algorithm enforces a strict rule that `reduced[i][j]` is never negative (`>= 0`).
This means that for **any** valid assignment you can possibly pick (optimal or not), the total cost must be at least the sum of the potentials.
`Total Cost >= Sum(u) + Sum(v)`

**The Perfect Score**
When we find an assignment where every link uses a **zero** in the reduced matrix:
1.  The sum of `reduced[i][j]` for our assignment is **0**.
2.  Therefore, our Total Cost is exactly `Sum(u) + Sum(v)`.

Since the cost can never be lower than the potentials, and we hit that exact number, we have mathematically proven that **no cheaper solution exists**.
