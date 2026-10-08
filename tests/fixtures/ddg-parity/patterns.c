/* Representative DDG patterns, including propagation across globals. */
int global_value = 13;
static int file_value = 3;
struct Pair { int left; int right; };
struct Pair global_pair = {1, 2};
int *global_pointer;
int initialized_global(void) { return global_value; }
int global_read_then_write(int x) { int y = global_value; global_value = x; return y + global_value; }
int global_branches(int x) { if (x) global_value = x; return global_value; }
int static_global_read(void) { return file_value; }
int member_global(int x) { global_pair.left = x; return global_pair.left + global_pair.right; }
int pointer_global(int x) { *global_pointer = x; return *global_pointer; }
int global_to_pointer(int *p) { global_pointer = p; return *global_pointer; }
int chained(int x) { int a, b; a = b = x; return a + b; }
int nested_compound(int x, int y) { x += (y += x); return x + y; }
int evaluation_sequence(int x) { int y = (x++, x + 1); return y + x; }
int mutate_pointer(int *p) { int x = (*p)++; return x + *p; }
int conditional_store(int x, int y) { int z; z = x ? (y = 1) : (y = 2); return z + y; }
int nested_shortcircuit(int x, int y, int z) { if (x && (y || z)) return y; return z; }
int loop_break(int x) { int y = 0; for (int i = 0; i < x; i++) { if (i == 3) break; y += i; } return y; }
int loop_continue(int x) { int y = 0; do { x--; if (x == 2) continue; y += x; } while (x > 0); return y; }
int switch_store(int x) { int y = 0; switch (x) { case 1: y = x; break; case 2: y += x; default: y++; } return y; }
int goto_join(int x) { int y; if (x) goto positive; y = 0; goto done; positive: y = x; done: return y; }
int early_returns(int x, int y) { if (x > 0) return x + y; if (y) return y; return 0; }
int constant_return(void) { return 7; }
void empty_definition(void) {}
int external(int);
int defined(int x) { return x + 1; }
int internal_chain(int x) { return defined(defined(x)); }
int external_chain(int x) { return external(external(x)); }
int external_result(int x) { int y = external(x); return y; }
int sizeof_array(int x) { int a[x]; return sizeof a; }
int matrix(int a[3][4], int i, int j, int x) { a[i][j] = x; return a[i][j]; }
int field_paths(struct Pair *p, int x) { p[0].left = x; return p[0].left + p[1].right; }
int equal_literals(int x) { x = 1; x = 1; return x; }
int equal_statements(int x, int c) { if (c) x = 1; else x = 1; return x; }
