# Tasks: Table operations (Phase 3)

## 0. Settle the open questions
- [ ] 0.1 Decide the join surface syntax (design Q9) and multiple-match behaviour (design Q10)

## 1. Grouping and joining (`evaluation`)
- [ ] 1.1 Native group by one or more columns (hash on key) with per-group aggregation
- [ ] 1.2 Native inner and left join on a key column (hash join)
- [ ] 1.3 Verify exactness is preserved through group, join, sort and selection

## 2. OMX (`omx-expressions`)
- [ ] 2.1 `group(<columns>)` pipe stage
- [ ] 2.2 Partitioned cursor offsets `[*±n by Column]`

## 3. Recalculation (`evaluation`)
- [ ] 3.1 Recalculate only the dependents of a changed input
- [ ] 3.2 Verify the result is identical to a full recalculation
