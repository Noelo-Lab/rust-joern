char *
two_lines(void);
static int
  *pointer_next_line
  (void);
int shared_a(int), *shared_b(char), **shared_c(void);
char *shared_p(void), shared_q(void), *shared_r(void);
size_t unknown_first(const char *, ...),
  unknown_second(char *);
extern void (*returns_hook (void)) (const char *, long);
void first_wins(int a);
void first_wins(long b);
void user(void) {
  extern void block_scope_first(int fd, const char *b);
  block_scope_first(1, 0);
}
void block_scope_first(int fd, const void *b);
int defined_later(int a);
int defined_later(int a) { return a; }
void user2(void) { int two_blocks(int inner); two_blocks(1); }
void user3(void) { int two_blocks(long inner2); two_blocks(2); }
