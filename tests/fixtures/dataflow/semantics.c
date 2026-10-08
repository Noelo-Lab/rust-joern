extern int sink(int);
void prototype(int, int);
void empty(int x, int y) {}
void body(int x, int y) { sink(x + y); }
int calls(int x, int y) { prototype(x, y); empty(x, y); body(x, y); return x; }
struct Container { int *items; };
int nested_items(struct Container *b, int i, int x) { b->items[i] = x; return b->items[i]; }
extern char *strncat(char *, const char *, long);
char *concat(char *a, const char *b, long n) { return strncat(a, b, n); }
int blockpath(int *p, int x) { ({ p; })[0] = x; return p[0]; }
