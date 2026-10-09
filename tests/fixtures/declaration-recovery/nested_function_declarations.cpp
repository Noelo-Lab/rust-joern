int work(int);
int valid_outer(int n) { int nested(int x) { return work(x); } return nested(n); }
int after_valid_outer(int n) { return work(n); }
int prototype_outer(int n) { int declared(int); return declared(n); }
int after_prototype_outer(int n) { return work(n); }
