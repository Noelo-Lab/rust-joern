int work(int);
int scalar_after(int n) {
  enum { GOOD=1, 2=2 };
  work(n);
  work(n+1);
  return n;
}
int if_after(int n) {
  enum { (-10)=-10 };
  if(n) work(n); else work(n+1);
  work(n+2);
  return n;
}
int block_after(int n) {
  enum { 2=2 };
  { work(n); work(n+1); }
  work(n+2);
  return n;
}
int enum_after(int n) {
  enum { 2=2 };
  enum { (-10)=-10 };
  int kept_after_second(int x) { return x; }
  work(n);
  return n;
}
int no_semicolon(int n) {
  enum { 2=2 }
  int skipped_without_semicolon(int x) { return x; }
  int kept_after_no_semicolon(int x) { return x; }
  return n;
}
int valid_enums(int n) {
  enum Valid { FIRST=(1+2), SECOND=3, };
  int valid_child(int x) { return x; }
  enum { THIRD=4 } value;
  int valid_after_value(int x) { return x; }
  work(n);
  return n;
}
