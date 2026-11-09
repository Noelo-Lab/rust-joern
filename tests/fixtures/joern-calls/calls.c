int tick(void);
int direct(int x) { return missing(x); }
int pointer(int (*fn)(int)) { return fn(tick()); }
struct Box { int (*fn)(int); };
int member(struct Box* p) { return p->fn(tick()); }
int grouped(int (*fn)(int)) { return (fn)(tick()); }
