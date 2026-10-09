int work(int);
int local_numeric(int n) {
  enum { 2 = 2 };
  int numeric_child(int x) { return work(x); }
  int numeric_kept(int x) { return x; }
  return numeric_kept(n);
}
int local_negative(int n) {
  enum { (-10) = -10 };
  int negative_child(int x) { return work(x); }
  int negative_kept(int x) { return x; }
  return negative_kept(n);
}
int local_prototypes(int n) {
  enum { 2 = 2 };
  int numeric_proto(int);
  enum { (-10) = -10 };
  int negative_proto(int);
  enum Valid { FIRST=1, SECOND=2 };
  int valid_proto(int);
  return n;
}
int after_complete(int n) { return n; }
int unfinished_outer(int n) {
  work(n);
  enum { 2 = 2 };
  int unfinished_numeric_child(int x) { return work(x); }
  int unfinished_numeric_kept(int x) { return x; }
  enum { (-10) = -10 };
  int unfinished_negative_proto(int);
  enum ValidAgain { THIRD=3 };
  int unfinished_valid_proto(int);
  return n;
