int step(int n);
int more(void);
void done(void);

/* The expression ends at the first `::` after a complete operand. */
void cast_compare(int n) { int r; r = 0 < (int)ns::step(n); }
void cast_assign(int n) { int r; r = (int)ns::kind::step(n); }
void plain_assign(int n) { int r; r = ns::step(n); }
void sum_assign(int n) { int r; r = n + ns::step(n); }
void template_assign(int n) { int r; r = (int)ns::box<char, int>::step(n); }
void indexed_assign(int *p) { int r; r = p[1]::step(*p); }

/* Its missing semicolon also fails the enclosing statement. */
void guarded(int n) { int r = 0; if (n) r = (int)ns::step(n); done(); }
void branches(int n) {
  int r;
  if (n) { r = (int)ns::step(n); } else { done(); }
}

/* A compound item `ns::f(x);` is the label `ns` with a problem statement. */
void bare_call(int n) { step(n); ns::step(n); done(); }
void loop_call(int n) { while (more()) { ns::step(n); } done(); }
void loop_calls(int n) { while (more()) { ns::step(n); ns::kind::step(n); } done(); }
void loop_mixed(int n) { while (more()) { ns::step(n); done(); } step(n); }
void branch_call(int n) { if (more()) { ns::step(n); } done(); }

/* Controls: an empty body or a plain label, and an unbraced loop body. */
void loop_empty(int n) { while (more()) { } step(n); }
void loop_label(int n) { while (more()) { next: ; } step(n); }
void loop_problem(int n) { while (more()) { n = *((n! + 4)); } step(n); }
void loop_unbraced(int n) { while (n) ns::step(n); done(); }
void conditional(int n) { int r; r = more() ? ns::step : 2; done(); }
