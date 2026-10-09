#if 1
#define ANNOTATE(n)
#define ZERO()
int ANNOTATE(1) ANNOTATE(int x) { if (x) return x; return 0; }
int after_one(int x) { if (x) return x; return 0; }
int ZERO() { while (x) x--; return x; }
int after_zero(int x) { return x + 1; }
#undef ANNOTATE
int ANNOTATE(int x) { return x; }
#endif
