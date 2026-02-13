# v0.1.13
+ major performance increase for closest and small improvement for intersection.

# v0.1.11
+ major performance increase: fix bug in eviction from queue for intersection. before this, intervals behind the query on the same chromosome were **not removed**

# v0.1.10
+ Fix bug in closest with n > 1 (#15)
+ Fix bug in reporting with intersect (#15)
+ Fix panic when --a-piece is None (make this an error at argument parsing time)
