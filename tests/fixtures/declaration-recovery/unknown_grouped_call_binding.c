int inferred_fresh(int n) { Unknown local; Unknown(other); return n; }
int unbound_fresh(int n) { Other(fresh); return n; }
int inferred_empty(int n) { Unknown local; Unknown(); return n; }
typedef int Actual;
int actual_fresh(int n) { Actual(other); return n; }
int actual_bound(int n) { Actual(n); return n; }
int actual_multiple(int n) { Actual(n, 0); return n; }
