struct S { int value; };
struct S (grouped(int n)) { struct S s={n}; return s; }
struct S (*factory(int n))[4] { return 0; }
int after_groups(int n) { return n; }
